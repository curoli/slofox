use std::{
    process::Command,
    sync::mpsc::{self, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

use serde_json::Value;

pub struct BrowserStream {
    pub application: String,
    pub title: String,
}

fn properties(node: &Value) -> &Value {
    &node["info"]["props"]
}

fn nodes(json: &str) -> Result<Vec<Value>, String> {
    serde_json::from_str(json).map_err(|error| format!("Invalid PipeWire graph: {error}"))
}

pub fn streams(json: &str) -> Result<Vec<BrowserStream>, String> {
    let mut streams: Vec<_> = nodes(json)?
        .iter()
        .filter_map(|node| {
            let props = properties(node);
            if node["type"] != "PipeWire:Interface:Node"
                || props["media.class"] != "Stream/Output/Audio"
            {
                return None;
            }
            Some(BrowserStream {
                application: props["application.name"].as_str()?.to_owned(),
                title: props["media.name"].as_str()?.to_owned(),
            })
        })
        .collect();
    streams.sort_by(|left, right| {
        (&left.application, &left.title).cmp(&(&right.application, &right.title))
    });
    streams
        .dedup_by(|left, right| left.application == right.application && left.title == right.title);
    Ok(streams)
}

pub fn routing_plan(
    json: &str,
    application: &str,
    title: &str,
    sink: &str,
) -> Result<Vec<u64>, String> {
    let objects = nodes(json)?;
    let sink_id = objects
        .iter()
        .find_map(|node| {
            let props = properties(node);
            let serial = match &props["object.serial"] {
                Value::Number(number) => number.to_string(),
                Value::String(serial) => serial.clone(),
                _ => String::new(),
            };
            (node["type"] == "PipeWire:Interface:Node"
                && props["media.class"] == "Audio/Sink"
                && (props["node.name"] == sink || serial == sink))
                .then(|| node["id"].as_u64())
                .flatten()
        })
        .ok_or_else(|| format!("Browser routing: output sink '{sink}' is unavailable"))?;
    let mut plan = Vec::new();
    for node in &objects {
        let props = properties(node);
        if node["type"] != "PipeWire:Interface:Node"
            || props["media.class"] != "Stream/Output/Audio"
            || props["application.name"] != application
            || props["media.name"] != title
        {
            continue;
        }
        let Some(id) = node["id"].as_u64() else {
            continue;
        };
        let destinations: Vec<_> = objects
            .iter()
            .filter_map(|link| {
                if link["type"] == "PipeWire:Interface:Link"
                    && link["info"]["output-node-id"].as_u64() == Some(id)
                {
                    link["info"]["input-node-id"].as_u64()
                } else {
                    None
                }
            })
            .collect();
        if destinations.is_empty()
            || destinations
                .iter()
                .any(|destination| *destination != sink_id)
        {
            plan.push(id);
        }
    }
    Ok(plan)
}

pub fn graph() -> Result<String, String> {
    let output = Command::new("pw-dump")
        .output()
        .map_err(|error| format!("Cannot run pw-dump: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Browser routing: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout).map_err(|error| format!("Invalid pw-dump output: {error}"))
}

pub fn route_once(application: &str, title: &str, sink: &str) -> Result<usize, String> {
    let plan = routing_plan(&graph()?, application, title, sink)?;
    for id in &plan {
        let output = Command::new("pw-metadata")
            .args([&id.to_string(), "target.object", sink, "Spa:String"])
            .output()
            .map_err(|error| format!("Cannot run pw-metadata: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "Cannot route browser stream: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
    }
    Ok(plan.len())
}

pub struct TabRouter {
    stop: Sender<()>,
    worker: Option<JoinHandle<()>>,
}

impl TabRouter {
    pub fn start(application: String, title: String, sink: String) -> Result<Self, String> {
        let (stop, stopped) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("browser-routing".into())
            .spawn(move || {
                let mut last_error = None;
                loop {
                    match route_once(&application, &title, &sink) {
                        Ok(count) => {
                            if count > 0 {
                                eprintln!(
                                    "Browser routing: moved {count} matching stream(s) to {sink}"
                                );
                            }
                            last_error = None;
                        }
                        Err(error) => {
                            if last_error.as_ref() != Some(&error) {
                                eprintln!("{error}");
                            }
                            last_error = Some(error);
                        }
                    }
                    if stopped.recv_timeout(Duration::from_secs(1))
                        != Err(mpsc::RecvTimeoutError::Timeout)
                    {
                        break;
                    }
                }
            })
            .map_err(|error| format!("Cannot start browser routing: {error}"))?;
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
}

impl Drop for TabRouter {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph(stream_id: u64, destination: u64) -> String {
        serde_json::json!([
            {"type":"PipeWire:Interface:Node","id":20,"info":{"props":{"media.class":"Audio/Sink","node.name":"browser","object.serial":100}}},
            {"type":"PipeWire:Interface:Node","id":stream_id,"info":{"props":{"media.class":"Stream/Output/Audio","application.name":"Firefox","media.name":"Talk show"}}},
            {"type":"PipeWire:Interface:Node","id":45,"info":{"props":{"media.class":"Stream/Output/Audio","application.name":"Firefox","media.name":"Other tab"}}},
            {"type":"PipeWire:Interface:Node","id":46,"info":{"props":{"media.class":"Stream/Output/Audio","application.name":"Chrome","media.name":"Talk show"}}},
            {"type":"PipeWire:Interface:Link","info":{"output-node-id":stream_id,"input-node-id":destination}},
            {"type":"PipeWire:Interface:Link","info":{"output-node-id":45,"input-node-id":50}},
            {"type":"PipeWire:Interface:Link","info":{"output-node-id":46,"input-node-id":50}}
        ]).to_string()
    }

    #[test]
    fn routes_only_the_selected_tab_and_application() {
        assert_eq!(
            routing_plan(&graph(40, 50), "Firefox", "Talk show", "browser").unwrap(),
            vec![40]
        );
        assert!(
            routing_plan(&graph(40, 50), "Firefox", "Unknown tab", "browser")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn recognizes_recreated_streams_and_already_correct_routes() {
        assert_eq!(
            routing_plan(&graph(99, 50), "Firefox", "Talk show", "100").unwrap(),
            vec![99]
        );
        assert!(
            routing_plan(&graph(99, 20), "Firefox", "Talk show", "browser")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn rejects_missing_sink_and_invalid_graph() {
        assert!(routing_plan(&graph(40, 50), "Firefox", "Talk show", "missing").is_err());
        assert!(routing_plan("[]", "Firefox", "Talk show", "browser").is_err());
        assert!(routing_plan("invalid", "Firefox", "Talk show", "browser").is_err());
    }

    #[test]
    fn lists_streams_for_selecting_exact_titles() {
        let streams = streams(&graph(40, 50)).unwrap();
        assert_eq!(streams.len(), 3);
        assert!(
            streams
                .iter()
                .any(|stream| stream.application == "Firefox" && stream.title == "Talk show")
        );
    }
}
