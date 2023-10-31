use std::{collections::HashMap, time::Duration};

use futures_util::StreamExt;
use ironhive_config::generate_agent_id;
use ironhive_core::{Agent, Ironhive, IronhiveRequest, IronhiveRespond, ScriptMode};
use tracing::{debug, info};
use tracing_test::traced_test;

#[traced_test]
#[tokio::test(flavor = "multi_thread")]
async fn run_script() {
    let server = nats_server::run_basic_server();

    let agent_id = generate_agent_id();

    info!("agent id: {}", agent_id.to_string());

    let agent = Agent::new(agent_id.to_string(), &server.client_url()).unwrap();

    let rpc = Ironhive::new(agent).await.unwrap();

    let client = rpc.client.clone();

    let req = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        client
            .publish_with_reply(
                agent_id.to_string(),
                "ironhive".into(),
                IronhiveRequest::RunScript {
                    code: r#"print("hi from ironhive!")"#.into(),
                    mode: ScriptMode::Binary {
                        path: "python3".into(),
                        ext: ".py".into(),
                    },
                    script_args: vec![],
                    timeout: Duration::from_secs(3),
                    env_vars: HashMap::new(),
                    id: 1,
                }
                .as_bytes(),
            )
            .await
            .unwrap();
        client
            .publish_with_reply(
                agent_id.to_string(),
                "ironhive".into(),
                IronhiveRequest::RunScript {
                    code: r#"
def fibonacci(n):
    if n <= 0:
        return "Please enter a positive integer."
    elif n == 1:
        return 0
    elif n == 2:
        return 1
    else:
        a, b = 0, 1
        for _ in range(3, n+1):
            a, b = b, a + b
        return b

n = 10
result = fibonacci(n)
print(f"The value of the {n}th term in the Fibonacci sequence is: {result}")
            "#
                    .into(),
                    mode: ScriptMode::Binary {
                        path: "python3".into(),
                        ext: ".py".into(),
                    },
                    script_args: vec![],
                    timeout: Duration::from_secs(10),
                    env_vars: HashMap::new(),
                    id: 2,
                }
                .as_bytes(),
            )
            .await
            .unwrap();
    });

    let client = rpc.client.clone();

    let res = async {
        let mut subscriber = client.subscribe("ironhive".into()).await.unwrap();

        let raw_resp = subscriber.next().await.unwrap();
        let resp = serde_json::from_slice::<IronhiveRespond>(&raw_resp.payload).unwrap();
        debug!("{resp:#?}");
        assert!(matches!(resp, IronhiveRespond::RunScriptResp { .. }));

        fn handle_resp(stdout: String, id: i32) {
            if id == 1 {
                assert_eq!(stdout.trim(), "hi from ironhive!");
            } else if id == 2 {
                assert_eq!(
                    stdout.trim(),
                    "The value of the 10th term in the Fibonacci sequence is: 34"
                );
            } else {
                panic!("Unknow resp");
            }
        }

        if let IronhiveRespond::RunScriptResp { stdout, id, .. } = resp {
            handle_resp(stdout, id);
        }

        let raw_resp = subscriber.next().await.unwrap();
        let resp = serde_json::from_slice::<IronhiveRespond>(&raw_resp.payload).unwrap();
        assert!(matches!(resp, IronhiveRespond::RunScriptResp { .. }));
        debug!("{resp:#?}");
        if let IronhiveRespond::RunScriptResp { stdout, id, .. } = resp {
            handle_resp(stdout, id);
        }
    };

    let tasks = async {
        let _ = tokio::join!(req, res);
    };

    tokio::select! {
        res = rpc.run() => res.unwrap(),
        _ = tasks => {
            info!("well");
        },
    }
}
