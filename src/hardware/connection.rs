use udev::Enumerator;

/// Recursively climbs the `udev` subsystem tree from a physical block device (e.g. `sda`)
/// to its parent controller, attempting to intercept and math-out literal USB bottleneck speeds.
pub fn get_connection_speed(device_name: &str) -> Option<String> {
    let mut enumerator = Enumerator::new().ok()?;
    enumerator.match_subsystem("block").ok()?;
    enumerator.match_sysname(device_name).ok()?;
    
    // Traverse the specific block hardware matches.
    for device in enumerator.scan_devices().ok()? {
        
        // Core Logic: Walk upwards targeting strictly the "usb" master subsystem root.
        if let Some(usb_parent) = device.parent_with_subsystem("usb").ok().flatten() {
            if let Some(speed_attr) = usb_parent.attribute_value("speed") {
                let speed_str = speed_attr.to_string_lossy().to_string();
                
                // Map the integer topology to the exact "Slow Lane" structures the LLM correlates securely.
                return match speed_str.as_str() {
                    "480" => Some("USB 2.0 (High-Speed)".to_string()),
                    "5000" => Some("USB 3.0/3.1 Gen 1 (SuperSpeed)".to_string()),
                    "10000" => Some("USB 3.1 Gen 2 (SuperSpeed+)".to_string()),
                    "20000" => Some("USB 3.2 Gen 2x2".to_string()),
                    _ => Some(format!("USB ({} Mbps)", speed_str)),
                };
            }
        }
    }
    
    // If it's pure PCIe/SATA and doesn't route through USB legacy logic.
    None
}