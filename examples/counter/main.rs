use async_trait::async_trait;
use mongodb::Client;
use ply_jobs::{schedule, Job, JobConfig, JobError, JobManager, MongoRepo};
use serde::{Deserialize, Serialize};
use std::process;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    simple_logger::init().unwrap();

    // Variant 1 - Use MongoDB
    let client = Client::with_uri_str("mongodb://localhost:27017")
        .await
        .unwrap();

    let repo = MongoRepo::new(client, "test");

    // Variant 2 - Use PickleDB
    // let db_client = PickleDb::new(
    //     "jobs.db",
    //     PickleDbDumpPolicy::AutoDump,
    //     SerializationMethod::Json,
    // );
    // let repo = PickleDbRepo::new(db_client);

    let mut manager = JobManager::new(process::id().to_string(), repo);

    let job = CountJob {
        client: reqwest::Client::new(),
    };
    let config = JobConfig::new("project-updater", schedule::minutely())
        .with_check_interval(Duration::from_secs(3));

    manager.register(config, job);

    let _ = manager.start_all();
    sleep(Duration::from_secs(120)).await;

    // manager
    //     .stop_by_name(String::from("project-updater"))
    //     .await
    //     .unwrap();
    // sleep(Duration::from_secs(30)).await;
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
struct State(i32);

#[derive(Clone)]
struct CountJob {
    client: reqwest::Client,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
struct Counter(i32);

#[async_trait]
impl Job for CountJob {
    async fn call(&mut self, state: Vec<u8>) -> std::result::Result<Vec<u8>, JobError> {
        let mut data: State = if state.len() == 0 {
            State(0)
        } else {
            serde_json::from_slice(&state).unwrap()
        };

        println!("Count: {}", data.0);

        sleep(Duration::from_secs(1)).await;

        data.0 += 1;

        // Do some async work so the job has some real IO going on
        match self
            .client
            .get("http://worldtimeapi.org/api/timezone/Europe/London.txt")
            .send()
            .await
            .map_err(JobError::any)?
            .text()
            .await
        {
            Ok(body) => {
                println!("Time in London: {:?}", body.lines().take(3).last().unwrap());
                Ok(serde_json::to_vec(&data).unwrap())
            }
            Err(e) => Err(JobError::any(e)),
        }
    }
}
