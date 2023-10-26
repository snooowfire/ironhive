# Ironhive

Ironhive is an agent software designed to forward messages using the NATS messaging service. It provides a convenient way to monitor and manage various aspects of a system through message-based communication. This README document provides an overview of Ironhive and explains how to install and use it.

## Table of Contents
- [Installation](#installation)
- [Usage](#usage)
- [Supported Functionality](#supported-functionality)
- [Contributing](#contributing)
- [License](#license)

## Installation

To install Ironhive, follow these steps:

1. Ensure that you have Rust programming language and Cargo package manager installed on your system.
2. Open a terminal or command prompt.
3. Run the following command to install Ironhive:

```shell
cargo install --git https://github.com/snooowfire/ironhive.git
```

## Usage

Once Ironhive is installed, you can use it by following these steps:

1. Initialize the configuration by running the following command:

```shell
ironhive install --nats-servers <NATS_SERVERS>
```

Replace `<NATS_SERVERS>` with the list of NATS server addresses you want to connect to.

2. Start the monitoring service by running the following command:

```shell
ironhive rpc
```

Note: On Windows, you may need to run the command with administrator privileges.

## Supported Functionality

Ironhive supports the following functionality:

- **Ping**: Ping message to check connectivity.
- **Patch Management**: Enable or disable patch management.
- **Processes**: Retrieve information about running processes.
- **Kill Process**: Terminate a specific process by its ID.
- **Raw Command**: Execute a shell command with optional timeout.
- **Windows Services**: Retrieve a list of Windows services.
- **Windows Service Detail**: Retrieve detailed information about a specific Windows service.
- **Windows Service Action**: Perform an action (start, stop, restart) on a Windows service.
- **Edit Windows Service**: Modify the start type of a Windows service.
- **Run Script**: Execute a script with optional timeout, arguments, and environment variables.
- **Software List**: Retrieve a list of installed software.
- **Reboot Now**: Initiate an immediate system reboot.
- **Needs Reboot**: Check if the system requires a reboot.
- **System Information**: Retrieve general system information.
- **WMI**: Execute a WMI (Windows Management Instrumentation) query.
- **CPU Load Average**: Retrieve the average CPU load.
- **CPU Usage**: Retrieve CPU usage information.
- **Public IP**: Retrieve the public IP address of the system.
- **Install Choco**: Install Chocolatey package manager.
- **Install With Choco**: Install a program using Chocolatey.
- **Get Windows Updates**: Retrieve a list of available Windows updates.
- **Install Windows Updates**: Install specified Windows updates.

Please note that some of the above functionality may not be implemented yet, and additional features will be added gradually in the future.

## Contributing

Contributions to Ironhive are welcome! If you would like to contribute to the project, please follow the guidelines outlined in the [CONTRIBUTING.md](CONTRIBUTING.md) file.

## License

Ironhive is open-source software released under the MIT License. See the [LICENSE](LICENSE) file for more details.