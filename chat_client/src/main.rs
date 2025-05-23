// Name: BaekSungHyun
// Student ID: 20220417
// “Network Applications and Design” Homework Assignment #4

use std::env;
use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::process;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// server configuration
const SERVER_ADDRESS: &str = "nsl5.cau.ac.kr";
const SERVER_PORT: u16 = 20417;

// Command codes - 1 byte encoding for commands
const CMD_LIST: u8 = 1;
const CMD_TO: u8 = 2;
const CMD_EXCEPT: u8 = 3;
const CMD_BAN: u8 = 4;
const CMD_PING: u8 = 5;
const CMD_EXIT: u8 = 6;
const CMD_CHAT: u8 = 7;

// Define a struct to hold client state
struct ClientState {
    connected: bool,
    nickname: String,
}

// Helper function to check for prohibited message content
fn contains_prohibited_content(content: &str) -> bool {
    content.to_lowercase().contains("i hate professor")
}

// Function to handle incoming messages from the server
fn receive_messages(stream: TcpStream, state: Arc<Mutex<ClientState>>) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    // save the nickname for later use
    let _my_nickname = state.lock().unwrap().nickname.clone();

    while state.lock().unwrap().connected {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => {
                // Connection closed by server
                println!("Disconnected from server.");
                state.lock().unwrap().connected = false;
                process::exit(0); // immadiately exit
            }
            Ok(_) => {
                // Print the received message without adding a newline
                let nickname = state.lock().unwrap().nickname.clone();

                if line.contains(&format!("{} left the room", nickname)) {
                    // ignore the message that the user left the room
                    continue;
                }

                println!("{}", line.trim_end());

                // Flush stdout to ensure the message is displayed immediately
                let _ = io::stdout().flush();

                // check for specific messages
                if line.contains("you are banned by")
                    || line.contains("You sent a prohibited message")
                {
                    println!("You have been removed from the chat room.");
                    state.lock().unwrap().connected = false;
                    process::exit(0); // 즉시 종료
                }
            }
            Err(e) => {
                eprintln!("Error reading from server: {}", e);
                state.lock().unwrap().connected = false;
                process::exit(1); // exit with error
            }
        }
    }
}

// Function to handle user input and send messages to the server
fn handle_user_input(
    mut stream: TcpStream,
    _nickname: &str,
    state: Arc<Mutex<ClientState>>,
) -> io::Result<()> {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    // add a thread to check the connection status
    let check_state = Arc::clone(&state);
    let check_stream = stream.try_clone()?;

    thread::spawn(move || {
        while check_state.lock().unwrap().connected {
            thread::sleep(Duration::from_millis(500));

            // check if the connection is still alive
            if let Err(_) = check_stream.peer_addr() {
                println!("\nLost connection to server. Terminating.");
                process::exit(0);
            }
        }
    });

    while state.lock().unwrap().connected {
        if let Some(line) = lines.next() {
            let input = line?;

            // Check for prohibited content in any input
            if contains_prohibited_content(&input) {
                println!(
                    "Warning: Your message contains a prohibited phrase. You will be disconnected."
                );
            }

            // Process the input
            if input.starts_with('\\') {
                // Command handling
                let parts: Vec<&str> = input.splitn(2, ' ').collect();
                let command = parts[0];

                match command {
                    "\\list" => {
                        stream.write_all(&[CMD_LIST])?;
                        stream.write_all(b"\n")?;
                        stream.flush()?;
                    }
                    "\\to" => {
                        if parts.len() < 2 {
                            println!("Usage: \\to <nickname> <message>");
                            continue;
                        }

                        let rest = parts[1];
                        let nick_msg: Vec<&str> = rest.splitn(2, ' ').collect();

                        if nick_msg.len() < 2 {
                            println!("Usage: \\to <nickname> <message>");
                            continue;
                        }

                        let target = nick_msg[0];
                        let message = nick_msg[1];

                        let full_msg = format!("{} {}", target, message);
                        stream.write_all(&[CMD_TO])?;
                        stream.write_all(full_msg.as_bytes())?;
                        stream.write_all(b"\n")?;
                        stream.flush()?;
                    }
                    "\\except" => {
                        if parts.len() < 2 {
                            println!("Usage: \\except <nickname> <message>");
                            continue;
                        }

                        let rest = parts[1];
                        let nick_msg: Vec<&str> = rest.splitn(2, ' ').collect();

                        if nick_msg.len() < 2 {
                            println!("Usage: \\except <nickname> <message>");
                            continue;
                        }

                        let target = nick_msg[0];
                        let message = nick_msg[1];

                        let full_msg = format!("{} {}", target, message);
                        stream.write_all(&[CMD_EXCEPT])?;
                        stream.write_all(full_msg.as_bytes())?;
                        stream.write_all(b"\n")?;
                        stream.flush()?;
                    }
                    "\\ban" => {
                        if parts.len() < 2 {
                            println!("Usage: \\ban <nickname>");
                            continue;
                        }

                        let target = parts[1].trim();
                        stream.write_all(&[CMD_BAN])?;
                        stream.write_all(target.as_bytes())?;
                        stream.write_all(b"\n")?;
                        stream.flush()?;
                    }
                    "\\ping" => {
                        let start = Instant::now();

                        // Send ping command
                        stream.write_all(&[CMD_PING])?;
                        stream.write_all(b"\n")?;
                        stream.flush()?;

                        // Wait for response and calculate RTT
                        thread::sleep(Duration::from_millis(100));
                        let elapsed = start.elapsed();
                        println!("Ping time: {:?}", elapsed);
                    }
                    _ => {
                        println!("invalid command");
                    }
                }
            } else {
                // if the input is not a command, send it as a message
                if !input.trim().is_empty() {
                    // send the message to the server
                    stream.write_all(&[CMD_CHAT])?;
                    stream.write_all(input.as_bytes())?;
                    stream.write_all(b"\n")?;
                    stream.flush()?;

                    if let Err(e) = stream.flush() {
                        eprintln!("Error sending message: {}", e);
                        state.lock().unwrap().connected = false;
                        process::exit(1);
                    }

                    // Check for prohibited content
                    if contains_prohibited_content(&input) {
                        thread::sleep(Duration::from_millis(500));
                    }
                }
            }
        }
    }

    Ok(())
}

// Setup a Ctrl+C handler
fn setup_ctrl_c_handler(stream: TcpStream) {
    let mut stream_clone = stream.try_clone().unwrap();

    ctrlc::set_handler(move || {
        // Send exit message to the server
        let _ = stream_clone.write_all(&[CMD_EXIT]);
        let _ = stream_clone.write_all(b"\n");
        let _ = stream_clone.flush();

        println!("\ngg~");
        process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");
}

fn main() -> io::Result<()> {
    // Get nickname from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <nickname>", args[0]);
        process::exit(1);
    }

    let nickname = &args[1];

    // Validate nickname
    if nickname.len() > 10 || nickname.contains(|c: char| !c.is_ascii_alphanumeric()) {
        eprintln!(
            "Nickname must be <= 10 characters, English only, no spaces or special characters"
        );
        process::exit(1);
    }

    // Connect to the server
    let server_addr = format!("{}:{}", SERVER_ADDRESS, SERVER_PORT);
    match TcpStream::connect(&server_addr) {
        Ok(stream) => {
            println!("Connected to server at {}", server_addr);

            // Set TCP_NODELAY to disable Nagle's algorithm
            if let Err(e) = stream.set_nodelay(true) {
                eprintln!("Warning: Failed to set TCP_NODELAY: {}", e);
            }

            // First, send the nickname
            let mut stream_clone = stream.try_clone()?;
            stream_clone.write_all(nickname.as_bytes())?;
            stream_clone.write_all(b"\n")?;
            stream_clone.flush()?;

            // Read the initial response
            let mut reader = BufReader::new(&stream);
            let mut response = String::new();
            reader.read_line(&mut response)?;

            // Check for error responses
            if response.contains("cannot connect") {
                println!("{}", response.trim());
                process::exit(1);
            }

            // Print the welcome message
            print!("{}", response);

            // Create shared state for the client
            let state = Arc::new(Mutex::new(ClientState {
                connected: true,
                nickname: nickname.clone(),
            }));

            // Setup Ctrl+C handler
            setup_ctrl_c_handler(stream.try_clone()?);

            // Spawn a thread to receive messages
            let receive_stream = stream.try_clone()?;
            let receive_state = Arc::clone(&state);
            thread::spawn(move || {
                receive_messages(receive_stream, receive_state);
            });

            // Handle user input
            handle_user_input(stream, nickname, state)?;
        }
        Err(e) => {
            eprintln!("Failed to connect to server: {}", e);
            process::exit(1);
        }
    }

    Ok(())
}
