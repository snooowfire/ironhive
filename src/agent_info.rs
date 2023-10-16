use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct CheckInNats {
    pub agent_id: String,
    pub version: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AgentInfoNats {
    pub agent_id: String,
    #[serde(rename = "logged_in_username")]
    pub username: String,
    pub hostname: String,
    #[serde(rename = "operating_system")]
    pub os: String,
    pub plat: String,
    pub total_ram: u64,
    pub boot_time: u64,
    #[serde(rename = "needs_reboot")]
    pub reboot_needed: bool,
    pub arch: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WinSvcNats {
    pub agent_id: String,
    #[serde(rename = "services")]
    pub win_svcs: Vec<WindowsService>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WindowsService {
    pub name: String,
    pub status: String,
    pub display_name: String,
    #[serde(rename = "binpath")]
    pub bin_path: String,
    pub description: String,
    pub username: String,
    pub pid: u32,
    pub start_type: String,
    #[serde(rename = "autodelay")]
    pub delayed_auto_start: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WinWMINats {
    pub agent_id: String,
    pub wmi: serde_json::Value, // Use serde_json::Value for dynamic deserialization
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WinDisksNats {
    pub agent_id: String,
    pub disks: Vec<Disk>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Disk {
    pub device: String,
    pub fstype: String,
    pub total: String,
    pub used: String,
    pub free: String,
    pub percent: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PublicIPNats {
    pub agent_id: String,
    pub public_ip: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WinSoftwareList {
    pub name: String,
    pub version: String,
    pub publisher: String,
    pub install_date: String,
    pub size: String,
    pub source: String,
    pub location: String,
    pub uninstall: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WinSoftwareNats {
    pub agent_id: String,
    pub software: Vec<WinSoftwareList>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;
    use serde_json::Value;

    #[test]
    fn test_check_in_nats_deserialization() {
        let json = r#"{
            "agent_id": "agent123",
            "version": "1.0"
        }"#;

        let deserialized: CheckInNats = serde_json::from_str(json).unwrap();

        assert_eq!(deserialized.agent_id, "agent123");
        assert_eq!(deserialized.version, "1.0");
    }

    #[test]
    fn test_agent_info_nats_deserialization() {
        let json = r#"{
            "agent_id": "agent123",
            "logged_in_username": "user123",
            "hostname": "localhost",
            "operating_system": "Windows",
            "plat": "x86_64",
            "total_ram": 8,
            "boot_time": 1633947600,
            "needs_reboot": false,
            "arch": "x86_64"
        }"#;

        let deserialized: AgentInfoNats = serde_json::from_str(json).unwrap();

        assert_eq!(deserialized.agent_id, "agent123");
        assert_eq!(deserialized.username, "user123");
        assert_eq!(deserialized.hostname, "localhost");
        assert_eq!(deserialized.os, "Windows");
        assert_eq!(deserialized.plat, "x86_64");
        assert_eq!(deserialized.total_ram, 8);
        assert_eq!(deserialized.boot_time, 1633947600);
        assert_eq!(deserialized.reboot_needed, false);
        assert_eq!(deserialized.arch, "x86_64");
    }

    #[test]
    fn test_win_svc_nats_deserialization() {
        let json = r#"{
            "agent_id": "agent123",
            "services": [
                {
                    "name": "Service1",
                    "status": "Running",
                    "display_name": "Display Name 1",
                    "binpath": "Bin Path 1",
                    "description": "Description 1",
                    "username": "User1",
                    "pid": 1234,
                    "start_type": "Automatic",
                    "autodelay": true
                },
                {
                    "name": "Service2",
                    "status": "Stopped",
                    "display_name": "Display Name 2",
                    "binpath": "Bin Path 2",
                    "description": "Description 2",
                    "username": "User2",
                    "pid": 5678,
                    "start_type": "Manual",
                    "autodelay": false
                }
            ]
        }"#;

        let deserialized: WinSvcNats = serde_json::from_str(json).unwrap();

        assert_eq!(deserialized.agent_id, "agent123");
        assert_eq!(deserialized.win_svcs.len(), 2);

        let service1 = &deserialized.win_svcs[0];
        assert_eq!(service1.name, "Service1");
        assert_eq!(service1.status, "Running");
        assert_eq!(service1.display_name, "Display Name 1");
        assert_eq!(service1.bin_path, "Bin Path 1");
        assert_eq!(service1.description, "Description 1");
        assert_eq!(service1.username, "User1");
        assert_eq!(service1.pid, 1234);
        assert_eq!(service1.start_type, "Automatic");
        assert_eq!(service1.delayed_auto_start, true);

        let service2 = &deserialized.win_svcs[1];
        assert_eq!(service2.name, "Service2");
        assert_eq!(service2.status, "Stopped");
        assert_eq!(service2.display_name, "Display Name 2");
        assert_eq!(service2.bin_path, "Bin Path 2");
        assert_eq!(service2.description, "Description 2");
        assert_eq!(service2.username, "User2");
        assert_eq!(service2.pid, 5678);
        assert_eq!(service2.start_type, "Manual");
        assert_eq!(service2.delayed_auto_start, false);
    }

    #[test]
    fn test_win_wmi_nats_deserialization() {
        let json = r#"{
            "agent_id": "agent123",
            "wmi": {
                "property1": "value1",
                "property2": "value2"
            }
        }"#;

        let deserialized: WinWMINats = serde_json::from_str(json).unwrap();

        assert_eq!(deserialized.agent_id, "agent123");
        assert_eq!(
            deserialized.wmi["property1"],
            Value::String("value1".to_string())
        );
        assert_eq!(
            deserialized.wmi["property2"],
            Value::String("value2".to_string())
        );
    }

    #[test]
    fn test_win_disks_nats_deserialization() {
        let json = r#"{
            "agent_id": "agent123",
            "disks": [
                {
                    "device": "/dev/sda",
                    "fstype": "ext4",
                    "total": "100GB",
                    "used": "50GB",
                    "free": "50GB",
                    "percent": 50
                },
                {
                    "device": "/dev/sdb",
                    "fstype": "ntfs",
                    "total": "200GB",
                    "used": "100GB",
                    "free": "100GB",
                    "percent": 50
                }
            ]
        }"#;

        let deserialized: WinDisksNats = serde_json::from_str(json).unwrap();

        assert_eq!(deserialized.agent_id, "agent123");
        assert_eq!(deserialized.disks.len(), 2);

        let disk1 = &deserialized.disks[0];
        assert_eq!(disk1.device, "/dev/sda");
        assert_eq!(disk1.fstype, "ext4");
        assert_eq!(disk1.total, "100GB");
        assert_eq!(disk1.used, "50GB");
        assert_eq!(disk1.free, "50GB");
        assert_eq!(disk1.percent, 50);

        let disk2 = &deserialized.disks[1];
        assert_eq!(disk2.device, "/dev/sdb");
        assert_eq!(disk2.fstype, "ntfs");
        assert_eq!(disk2.total, "200GB");
        assert_eq!(disk2.used, "100GB");
        assert_eq!(disk2.free, "100GB");
        assert_eq!(disk2.percent, 50);
    }

    #[test]
    fn test_public_ip_nats_deserialization() {
        let json = r#"{
            "agent_id": "agent123",
            "public_ip": "192.168.0.1"
        }"#;

        let deserialized: PublicIPNats = serde_json::from_str(json).unwrap();

        assert_eq!(deserialized.agent_id, "agent123");
        assert_eq!(deserialized.public_ip, "192.168.0.1");
    }

    #[test]
    fn test_win_software_list_deserialization() {
        let json = r#"{
            "name": "Software1",
            "version": "1.0",
            "publisher": "Publisher1",
            "install_date": "2022-01-01",
            "size": "100MB",
            "source": "Source1",
            "location": "Location1",
            "uninstall": "Uninstall1"
        }"#;

        let deserialized: WinSoftwareList = serde_json::from_str(json).unwrap();

        assert_eq!(deserialized.name, "Software1");
        assert_eq!(deserialized.version, "1.0");
        assert_eq!(deserialized.publisher, "Publisher1");
        assert_eq!(deserialized.install_date, "2022-01-01");
        assert_eq!(deserialized.size, "100MB");
        assert_eq!(deserialized.source, "Source1");
        assert_eq!(deserialized.location, "Location1");
        assert_eq!(deserialized.uninstall, "Uninstall1");
    }

    #[test]
    fn test_win_software_nats_deserialization() {
        let json = r#"{
            "agent_id": "agent123",
            "software": [
                {
                    "name": "Software1",
                    "version": "1.0",
                    "publisher": "Publisher1",
                    "install_date": "2022-01-01",
                    "size": "100MB",
                    "source": "Source1",
                    "location": "Location1",
                    "uninstall": "Uninstall1"
                },
                {
                    "name": "Software2",
                    "version": "2.0",
                    "publisher": "Publisher2",
                    "install_date": "2022-02-01",
                    "size": "200MB",
                    "source": "Source2",
                    "location": "Location2",
                    "uninstall": "Uninstall2"
                }
            ]
        }"#;

        let deserialized: WinSoftwareNats = serde_json::from_str(json).unwrap();

        assert_eq!(deserialized.agent_id, "agent123");
        assert_eq!(deserialized.software.len(), 2);

        let software1 = &deserialized.software[0];
        assert_eq!(software1.name, "Software1");
        assert_eq!(software1.version, "1.0");
        assert_eq!(software1.publisher, "Publisher1");
        assert_eq!(software1.install_date, "2022-01-01");
        assert_eq!(software1.size, "100MB");
        assert_eq!(software1.source, "Source1");
        assert_eq!(software1.location, "Location1");
        assert_eq!(software1.uninstall, "Uninstall1");

        let software2 = &deserialized.software[1];
        assert_eq!(software2.name, "Software2");
        assert_eq!(software2.version, "2.0");
        assert_eq!(software2.publisher, "Publisher2");
        assert_eq!(software2.install_date, "2022-02-01");
        assert_eq!(software2.size, "200MB");
        assert_eq!(software2.source, "Source2");
        assert_eq!(software2.location, "Location2");
        assert_eq!(software2.uninstall, "Uninstall2");
    }
}
