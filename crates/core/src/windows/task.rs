// use tracing::{debug, error, info};

// use crate::{AutomatedTask, Error};

// async fn run_task(task: AutomatedTask) -> Result<(), Error> {
//     let mut outputs = vec![];
//     for action in task.task_actions {
//         debug!("will run action: {action:?}");

//         let action_start = std::time::Instant::now();

//         let res = match action {
//             crate::TaskAction::CmdScript(cmd) => cmd.run().await,
//             crate::TaskAction::CmdShell(cmd) => cmd.run().await,
//         };

//         let action_exec_time = std::time::Instant::now().duration_since(action_start);

//         info!("Execution Time: {action_exec_time:?}");

//         let is_err = res.as_ref().map(|o| !o.stderr.is_empty()).unwrap_or(true);

//         match res {
//             Ok(output) => {
//                 outputs.push(output);
//             }
//             Err(e) => {
//                 error!("script action failed: {e:?}");
//             }
//         }

//         if !task.continue_on_error && is_err {
//             break;
//         }
//     }

//     Ok(())
// }

// TODO:
