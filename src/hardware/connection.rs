use crate::models::TopologyNode;
use udev::Enumerator;

/// Recursively climbs the `udev` subsystem tree from a physical block device (e.g. `sda`)
/// to its parent controller, capturing the full granular lineage for UI visualization.
/// Returns a tuple containing the (Bottleneck Description, Physical Node Path, Vector of Topology Nodes).
pub fn get_device_topology(device_name: &str) -> (Option<String>, String, Vec<TopologyNode>) {
    let mut nodes = Vec::new();
    let mut connection_summary = None;
    let mut physical_path = String::new();

    let mut enumerator = match Enumerator::new() {
        Ok(e) => e,
        Err(_) => return (None, physical_path, nodes),
    };

    let _ = enumerator.match_subsystem("block");
    let _ = enumerator.match_sysname(device_name);

    let devices = match enumerator.scan_devices() {
        Ok(d) => d,
        Err(_) => return (None, physical_path, nodes),
    };

    for device in devices {
        if physical_path.is_empty() {
            if let Some(sysp) = device.syspath().to_str() {
                physical_path = sysp.to_string();
            }
        }

        let mut current = Some(device);

        while let Some(dev) = current {
            let subsystem = dev
                .subsystem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let sysname = dev.sysname().to_string_lossy().to_string();

            nodes.push(TopologyNode {
                level: 0, // We will calculate the reversed level after the walk
                subsystem: subsystem.clone(),
                sysname,
            });

            // If we hit the USB layer, capture the bottleneck metrics for the summary string
            if subsystem == "usb" && connection_summary.is_none() {
                if let Some(speed_attr) = dev.attribute_value("speed") {
                    let speed_str = speed_attr.to_string_lossy();
                    connection_summary = match speed_str.as_ref() {
                        "480" => Some("USB 2.0 (High-Speed)".to_string()),
                        "5000" => Some("USB 3.0/3.1 Gen 1 (SuperSpeed)".to_string()),
                        "10000" => Some("USB 3.1 Gen 2 (SuperSpeed+)".to_string()),
                        "20000" => Some("USB 3.2 Gen 2x2".to_string()),
                        _ => Some(format!("USB ({} Mbps)", speed_str)),
                    };
                }
            }

            current = dev.parent();
        }
    }

    // Reverse the nodes so they go from Root (Level 1) to Leaf (Level N)
    nodes.reverse();
    for (idx, node) in nodes.iter_mut().enumerate() {
        node.level = idx + 1;
    }

    (connection_summary, physical_path, nodes)
}
