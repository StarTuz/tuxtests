//! Integration Test Harness for TuxTests Hardware Logic
use tuxtests::models::DriveInfo;

#[test]
fn test_slow_lane_nvme_adapter() {
    let raw = include_str!("fixtures/slow_lane.json");
    let drive: DriveInfo =
        serde_json::from_str(raw).expect("Failed to parse Slow Lane JSON into DriveInfo");

    assert_eq!(drive.drive_type, "NVMe");
    assert_eq!(drive.connection, "USB 2.0 (High-Speed)");
    assert_eq!(drive.serial.unwrap(), "EXT_NVME_001");
    println!("NVMe over USB slow lane securely typed and parsed.");
}

#[test]
fn test_zombie_drive_smartctl_failure() {
    let raw = include_str!("fixtures/zombie_drive.json");
    let drive: DriveInfo = serde_json::from_str(raw).expect("Failed to parse Zombie Drive JSON");

    assert_eq!(drive.smartctl_exit_code, Some(4));
    assert_eq!(drive.health_ok, false);
    assert_eq!(
        drive.physical_path,
        "/devices/pci0000:00/0000:00:1f.2/ata2/host1/target1:0:0/1:0:0:0"
    );
    println!("Zombie drive edge case explicitly validated.");
}

#[test]
fn test_lvm_on_luks() {
    let raw = include_str!("fixtures/lvm_on_luks.json");
    let drive: DriveInfo = serde_json::from_str(raw).expect("Failed to parse LVM on LUKS JSON");

    assert_eq!(drive.parent.unwrap(), "nvme0n1p3");
    assert_eq!(drive.is_luks, Some(true));
    assert_eq!(drive.drive_type, "LVM");
    println!("LUKS and nested Mapper device parsed without Option<T> explosion.");
}
