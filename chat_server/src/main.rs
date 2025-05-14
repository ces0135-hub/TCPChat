// Name: BaekSungHyun
// Student ID: 20220417
// “Network Applications and Design” Homework Assignment #4

use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

// Command codes - 1 byte encoding for commands
const CMD_LIST: u8 = 1;
const CMD_TO: u8 = 2;
const CMD_EXCEPT: u8 = 3;
const CMD_BAN: u8 = 4;
const CMD_PING: u8 = 5;
const CMD_EXIT: u8 = 6;
const CMD_CHAT: u8 = 7;

// Maximum number of clients allowed
const MAX_CLIENTS: usize = 4;

// Port to listen on - replace with your designated port number
const PORT: u16 = 8080;

// Structure to store client information
struct Client {
    nickname: String,
    stream: TcpStream,
    ip: String,
    port: u16,
}

impl Client {
    fn new(nickname: String, stream: TcpStream) -> Self {
        let peer_addr = stream.peer_addr().unwrap();
        let ip = peer_addr.ip().to_string();
        let port = peer_addr.port();

        Client {
            nickname,
            stream,
            ip,
            port,
        }
    }

    // send a message to the client
    fn send_message(&self, message: &str) -> io::Result<()> {
        let mut stream = self.stream.try_clone()?;
        stream.write_all(message.as_bytes())?;
        stream.flush()?;
        Ok(())
    }
}

// Helper function to check for prohibited message content
fn contains_prohibited_content(content: &str) -> bool {
    content.to_lowercase().contains("i hate professor")
}

// send a message to all clients
fn broadcast_to_all(clients: &HashMap<String, Client>, message: &str, except: Option<&str>) {
    for (nickname, client) in clients.iter() {
        if let Some(except_nick) = except {
            if nickname == except_nick {
                continue;
            }
        }

        if let Err(e) = client.send_message(message) {
            eprintln!("Error broadcasting to {}: {}", nickname, e);
        }
    }
}

// Handle user disconnection due to prohibited content
fn disconnect_for_prohibited_content(
    stream: &TcpStream,
    nickname: &str,
    clients: &Arc<Mutex<HashMap<String, Client>>>,
) -> io::Result<()> {
    // Notify the client being disconnected
    let mut banned_stream = stream.try_clone()?;
    banned_stream
        .write_all("You sent a prohibited message and will be disconnected.\n".as_bytes())?;
    banned_stream.flush()?;

    // Notify other clients and remove from client list
    {
        let mut clients_lock = clients.lock().unwrap();
        let num_remaining = clients_lock.len() - 1;

        // Message for other clients
        let notify_msg = format!(
            "[{} was removed for prohibited message. {} users remain.]\n",
            nickname, num_remaining
        );

        // Send to all other clients
        for (nick, client) in clients_lock.iter() {
            if *nick != nickname {
                let _ = client.send_message(&notify_msg);
            }
        }

        // Remove from client list
        clients_lock.remove(nickname);

        println!(
            "{} is removed for sending prohibited message. There are {} users now",
            nickname, num_remaining
        );
    }

    Ok(())
}

// Process incoming messages from clients
fn handle_client(
    stream: TcpStream,
    nickname: String,
    clients: Arc<Mutex<HashMap<String, Client>>>,
) -> io::Result<()> {
    let client_addr = stream.peer_addr()?;

    // print client connection information
    {
        let clients_lock = clients.lock().unwrap();
        let num_users = clients_lock.len();
        println!(
            "{} joined from {}:{}. There are {} users in the room",
            nickname,
            client_addr.ip(),
            client_addr.port(),
            num_users
        );
    }
    {
        let num_users = clients.lock().unwrap().len();
        println!(
            "{} joined from {}:{}. There are {} users in the room",
            nickname,
            client_addr.ip(),
            client_addr.port(),
            num_users
        );
    }

    // send welcome message to the new client
    {
        let num_users = clients.lock().unwrap().len();
        let welcome_msg = format!(
            "[Welcome {} to CAU net-class chat room at nsl5.cau.ac.kr:{}. There are {} users in the room.]\n",
            nickname, PORT, num_users
        );

        let mut stream_clone = stream.try_clone()?;
        stream_clone.write_all(welcome_msg.as_bytes())?;
        stream_clone.flush()?;
    }

    // broadcast to all clients that a new user has joined
    {
        let clients_lock = clients.lock().unwrap();
        let num_users = clients_lock.len();
        let join_msg = format!(
            "[{} joined from {}:{}. There are {} users in the room.]\n",
            nickname,
            client_addr.ip(),
            client_addr.port(),
            num_users
        );

        broadcast_to_all(&clients_lock, &join_msg, Some(&nickname));
    }

    // set up a buffered reader for the client stream
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut buffer = Vec::new();

    // main loop to read messages from the client
    loop {
        buffer.clear();
        let bytes_read = reader.read_until(b'\n', &mut buffer)?;

        // check for end of stream
        if bytes_read == 0 {
            break;
        }

        // skip empty messages or messages with only the command byte
        if buffer.is_empty() || bytes_read <= 1 {
            continue;
        }

        // first byte is the command, rest is the content
        let cmd = buffer[0];
        let content = String::from_utf8_lossy(&buffer[1..bytes_read - 1]).to_string();

        // debug print the received message
        println!(
            "Received from {}: cmd={}, content='{}', bytes={}",
            nickname, cmd, content, bytes_read
        );

        // Check for prohibited content regardless of command type
        if contains_prohibited_content(&content) {
            disconnect_for_prohibited_content(&stream, &nickname, &clients)?;
            return Ok(());
        }

        match cmd {
            CMD_CHAT => {
                // print the chat message
                println!("{}: {}", nickname, content);

                // broadcast the message to all clients
                {
                    let clients_lock = clients.lock().unwrap();
                    let msg = format!("{}> {}\n", nickname, content);
                    broadcast_to_all(&clients_lock, &msg, Some(&nickname));
                }
            }
            CMD_LIST => {
                let clients_lock = clients.lock().unwrap();
                let mut list_msg = String::new();

                // message format
                list_msg.push_str("Connected users:\n");
                for (nick, client) in clients_lock.iter() {
                    list_msg.push_str(&format!("{}, {}, {}\n", nick, client.ip, client.port));
                }

                // send the message to the requesting client
                if let Some(client) = clients_lock.get(&nickname) {
                    let _ = client.send_message(&list_msg);
                }
            }
            CMD_TO => {
                // format: nickname message
                if let Some(space_idx) = content.find(' ') {
                    let target = &content[..space_idx];
                    let message = &content[space_idx + 1..];

                    let clients_lock = clients.lock().unwrap();

                    if clients_lock.contains_key(target) {
                        // send the message to the target user
                        if let Some(client) = clients_lock.get(target) {
                            let msg = format!("from: {}> {}\n", nickname, message);
                            let _ = client.send_message(&msg);
                        }
                    } else {
                        // error message if target user does not exist
                        if let Some(client) = clients_lock.get(&nickname) {
                            let error_msg = format!("Error: User '{}' does not exist.\n", target);
                            let _ = client.send_message(&error_msg);
                        }
                    }
                }
            }
            CMD_EXCEPT => {
                if let Some(space_idx) = content.find(' ') {
                    let except_nick = &content[..space_idx];
                    let message = &content[space_idx + 1..];

                    if except_nick == nickname {
                        // 자기 자신 제외: 잘못된 명령으로 간주
                        println!("invalid command: \\except {} {}", except_nick, message);
                        if let Some(client) = clients.lock().unwrap().get(&nickname) {
                            let _ = client.send_message("invalid command\n");
                        }
                    } else {
                        let clients_lock = clients.lock().unwrap();
                        if clients_lock.contains_key(except_nick) {
                            for (nick, client) in clients_lock.iter() {
                                if *nick != nickname && *nick != except_nick {
                                    let msg = format!("{}> {}\n", nickname, message);
                                    let _ = client.send_message(&msg);
                                }
                            }
                        } else {
                            if let Some(client) = clients_lock.get(&nickname) {
                                let error_msg =
                                    format!("Error: User '{}' does not exist.\n", except_nick);
                                let _ = client.send_message(&error_msg);
                            }
                        }
                    }
                } else {
                    // invaild \except command
                    println!("invalid command: \\except {}", content);
                    if let Some(client) = clients.lock().unwrap().get(&nickname) {
                        let _ = client.send_message("invalid command\n");
                    }
                }
            }
            CMD_BAN => {
                let ban_nick = content.trim();
                let mut clients_lock = clients.lock().unwrap();

                if ban_nick != nickname && clients_lock.contains_key(ban_nick) {
                    // ban the user
                    if let Some(client) = clients_lock.get(ban_nick) {
                        let ban_msg = format!("you are banned by {}\n", nickname);
                        let _ = client.send_message(&ban_msg);
                    }

                    // remove the banned user from the client list
                    clients_lock.remove(ban_nick);

                    // broadcast the ban message to all clients
                    let ban_broadcast = format!(
                        "[{} left the room. There are {} users now]\n",
                        ban_nick,
                        clients_lock.len()
                    );

                    println!(
                        "{} was banned by {}. There are {} users now",
                        ban_nick,
                        nickname,
                        clients_lock.len()
                    );

                    broadcast_to_all(&clients_lock, &ban_broadcast, None);
                } else if ban_nick == nickname {
                    // can't ban client itself
                    if let Some(client) = clients_lock.get(&nickname) {
                        let error_msg = "Error: You cannot ban yourself.\n";
                        let _ = client.send_message(error_msg);
                    }
                } else {
                    // no such user
                    if let Some(client) = clients_lock.get(&nickname) {
                        let error_msg = format!("Error: User '{}' does not exist.\n", ban_nick);
                        let _ = client.send_message(&error_msg);
                    }
                }
            }
            CMD_PING => {
                let start = Instant::now();
                let clients_lock = clients.lock().unwrap();

                if let Some(client) = clients_lock.get(&nickname) {
                    // send ping message
                    let rtt_msg = format!("RTT: {:?}\n", start.elapsed());
                    let _ = client.send_message(&rtt_msg);
                }
            }
            CMD_EXIT => {
                // disconnect the client
                let mut clients_lock = clients.lock().unwrap();
                clients_lock.remove(&nickname);

                let exit_msg = format!(
                    "[{} left the room. There are {} users now]\n",
                    nickname,
                    clients_lock.len()
                );

                println!(
                    "{} left the room. There are {} users now",
                    nickname,
                    clients_lock.len()
                );

                // broadcast the exit message to all clients
                broadcast_to_all(&clients_lock, &exit_msg, None);

                break;
            }
            _ => {
                // invaild or wrong format command
                println!("invalid command: \\unknown {}", content);
                if let Some(client) = clients.lock().unwrap().get(&nickname) {
                    let _ = client.send_message("invalid command\n");
                }
            }
        }
    }

    // disconnect the client
    let mut clients_lock = clients.lock().unwrap();
    clients_lock.remove(&nickname);

    println!(
        "{} disconnected. There are {} users now",
        nickname,
        clients_lock.len()
    );

    let leave_msg = format!(
        "[{} left the room. There are {} users now]\n",
        nickname,
        clients_lock.len()
    );

    // broadcast the leave message to all clients
    broadcast_to_all(&clients_lock, &leave_msg, None);

    Ok(())
}

fn main() -> io::Result<()> {
    // generate a random port number
    let listener = TcpListener::bind(format!("0.0.0.0:{}", PORT))?;
    println!("Server listening on port {}", PORT);

    // save the clients in a thread-safe structure
    let clients: Arc<Mutex<HashMap<String, Client>>> = Arc::new(Mutex::new(HashMap::new()));

    // accept incoming connections
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let client_addr = stream.peer_addr()?;
                println!(
                    "New connection from {}:{}",
                    client_addr.ip(),
                    client_addr.port()
                );

                // check if the maximum number of clients is reached
                let current_clients = clients.lock().unwrap().len();
                if current_clients >= MAX_CLIENTS {
                    let error_msg = "chatting room full. cannot connect\n";
                    let mut stream_clone = stream.try_clone()?;
                    stream_clone.write_all(error_msg.as_bytes())?;
                    stream_clone.flush()?;
                    println!(
                        "Connection from {}:{} rejected: chatting room full (max {} clients)",
                        client_addr.ip(),
                        client_addr.port(),
                        MAX_CLIENTS
                    );
                    continue;
                }

                // read the nickname from the client
                let mut reader = BufReader::new(&stream);
                let mut nickname = String::new();
                reader.read_line(&mut nickname)?;
                nickname = nickname.trim().to_string();

                // check the nickname format
                if nickname.len() > 10 || nickname.contains(|c: char| !c.is_ascii_alphanumeric()) {
                    let error_msg = "nickname must be <= 10 characters, English only, no spaces or special chars\n";
                    let mut stream_clone = stream.try_clone()?;
                    stream_clone.write_all(error_msg.as_bytes())?;
                    stream_clone.flush()?;
                    println!(
                        "Connection from {}:{} rejected: invalid nickname format '{}'",
                        client_addr.ip(),
                        client_addr.port(),
                        nickname
                    );
                    continue;
                }

                // check if the nickname is already in use
                let mut clients_lock = clients.lock().unwrap();
                if clients_lock.contains_key(&nickname) {
                    let error_msg = "nickname already used by another user. cannot connect\n";
                    let mut stream_clone = stream.try_clone()?;
                    stream_clone.write_all(error_msg.as_bytes())?;
                    stream_clone.flush()?;
                    println!(
                        "Connection from {}:{} rejected: nickname '{}' already in use",
                        client_addr.ip(),
                        client_addr.port(),
                        nickname
                    );
                    continue;
                }

                // add the new client to the list
                let client = Client::new(nickname.clone(), stream.try_clone()?);
                clients_lock.insert(nickname.clone(), client);
                drop(clients_lock);

                // spawn a new thread to handle the client
                let clients_clone = Arc::clone(&clients);
                thread::spawn(move || {
                    if let Err(e) = handle_client(stream, nickname.clone(), clients_clone) {
                        eprintln!("Error handling client {}: {}", nickname, e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Connection failed: {}", e);
            }
        }
    }

    Ok(())
}
