# Secure TCP/TLS WebSocket Server  

A high-performance, scalable WebSocket server designed for secure real-time communication. Built with Rust and powered by tokio, this server supports secure TLS connections using tokio-rustls, ensuring encrypted communication over WebSockets. It includes built-in rate limiting to prevent abuse, health monitoring through an HTTP metrics endpoint, and a graceful shutdown mechanism for clean termination. Configurable via environment variables, this WebSocket server is optimized for performance, making it ideal for applications requiring reliable, low-latency communication, such as financial systems, gaming, and collaborative platforms. 

## Features  
- **Secure TLS Support**: Uses `tokio-rustls` for encrypted WebSocket connections.  
- **Rate Limiting**: Prevents abuse by limiting incoming connection rates.  
- **Graceful Shutdown**: Handles termination signals for a clean exit.  
- **Configuration via Environment**: Reads settings from `.env` or environment variables.  
- **Health Monitoring**: Exposes an HTTP metrics endpoint for monitoring.  

## Getting Started  

### Prerequisites  
- Rust (latest stable version recommended)  
- OpenSSL (for generating TLS certificates)  

### Installation  
1. Clone this repository:  
   ```sh
   git clone https://github.com/Guap-Codes/websocket-rs.git

   cd websocket-rs
```
2. Install dependencies:
   ```sh
    cargo build --release
  ```

### Configuration

1. TLS Certificate Generation
  - Generate self-signed certificates (required for TLS):
  ```sh
  mkdir -p certs

  openssl req -x509 -newkey rsa:4096 -keyout certs/key.pem -out certs/cert.pem \ -days 365 -nodes -subj "/CN=localhost
  ```

2. Set up a .env file to use environment variables:
```
export WS_PORT=8080
export WS_MAX_CONNECTIONS=100       
export WS_MESSAGE_RATE_LIMIT=100
export WS_ENABLE_COMPRESSION=true   
export WS_ENABLE_TLS=true
export WS_TLS_CERT_PATH=certs/cert.pem
export WS_TLS_KEY_PATH=certs/key.pem
```

### Running the Server

Start the server with:
```sh
cargo run --release
```
By default, the server listens on 0.0.0.0:PORT.

### API Endpoints

1. WebSocket Server: Connect via ws://localhost:<PORT> or wss://localhost:<PORT> for TLS.
- Connect with no verification:
    ```sh
    wscat -c wss://localhost:8080 -no-check

    ```
- Connect with TLS verification:
    ```sh
    wscat -c wss://localhost:8080 --ca certs/cert.pem
    ```
- Example messages to send after connection:
    **Authentication**
    {"type":"Auth","token":"your_access_token"}

    **Text message**
    {"type":"Text","content":"Hello World","compressed":false}

    **Command**
    {"type":"Command","action":"ping","parameters":{}}    

    
2. Health Metrics: Exposed via HTTP (configurable endpoint).
    ```sh
    curl -s http://localhost:9080/metrics
    ```

### Benchmarks

This project includes a Criterion benchmark suite. Run benchmarks using:
 ```sh
 cargo bench
  ```

### Contributing

1. Fork the repository
2. Create a feature branch (git checkout -b feature-name)
3. Commit changes (git commit -m "Add new feature")
4. Push to the branch (git push origin feature-name)
5. Open a Pull Request

### License

This project is licensed under the MIT License.