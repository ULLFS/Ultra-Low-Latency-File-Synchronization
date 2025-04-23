# Ultra-Low Latency File Synchronization

## Project Overview

The increasing demand for real-time data synchronization across devices and remote systems has highlighted the need for efficient, low-latency file synchronization solutions. Traditional methods often suffer from high latency and significant overhead, which are unacceptable in many modern applications. This project aims to address these challenges by leveraging cutting-edge technologies like eBPF for file change detection and Rust for robust, performant system design.

## Project Description

This project involves developing an ultra-low latency file synchronization solution using the `steady_state` actor-based framework in Rust. The primary goal is to monitor changes in a local folder and synchronize those changes to a remote folder with minimal delay.

### Key Features

- **Real-Time File Change Detection:** Utilize eBPF (Extended Berkeley Packet Filter) to monitor file changes efficiently and trigger synchronization events instantly.
- **Rust-Based Implementation:** Develop the solution using Rust and the `steady_state` crate to ensure high performance, reliability, and safety.
- **Delta-Based Synchronization:** Implement a mechanism to detect and transmit only the changes (deltas) in files, reducing the amount of data transferred and minimizing latency.
- **Secure Communication:** Use the `stun-rs` crate to establish secure remote connections between instances, ensuring data integrity and privacy through strong encryption.
- **Optional Compression:** Explore the possibility of adding compression to further optimize data transfer, with a plugin-based approach to enable file type-specific compression.

## Technical Requirements

- **Language and Framework:** The project will be developed in Rust, utilizing the `steady_state` crate for actor-based concurrency.
- **Monitoring Technology:** Incorporate eBPF for efficient file change detection.
- **Communication:** Utilize the `stun-rs` crate for secure instance communication.
- **License:** The project will be released under the MIT license.
- **Platform Compatibility:** The software must be compatible with Linux and run as a systemd process.
- **Testing:** A comprehensive suite of unit and integration tests will be developed.

## Presentation and Community Engagement

The project will be presented as a code review session at the STL Rust meetup, in addition to standard presentations at Southern Illinois University Edwardsville (SIUE). This offers a valuable opportunity to engage with the broader Rust community and receive feedback from experienced developers.

## Intellectual Property Rights

This is an open-source project under the MIT license, a common practice for Rust crates. The intellectual property rights will be shared equally between the student development team and the client. Both parties will have the right to independently use, modify, and distribute the resultant product.

## Why This Project is Exciting

This project offers an excellent opportunity to work with cutting-edge technologies and gain experience in multiple critical areas of modern software development:

- **Hands-On with eBPF:** Learn and implement eBPF for efficient file monitoring (this will require some C programming).
- **Master Rust and Actor-Based Concurrency:** Gain expertise in Rust programming and the `steady_state` crate.
- **Security and Optimization:** Implement robust security measures and explore advanced optimization techniques like delta synchronization and data compression.
- **Real-World Impact:** Develop a solution with significant practical applications, from cloud storage synchronization to real-time data replication in enterprise environments.
- **Community Engagement:** Showcase your work at the STL Rust meetup, engaging with the broader Rust community and receiving valuable feedback.

This project is not just a technical challenge but a chance to make a meaningful contribution to the open-source community and enhance your professional portfolio with a highly relevant and impactful software solution.
