// Name: BaekSungHyun
// Student ID: 20220417

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
const PORT: u16 = 8080; // Replace with your designated port

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
    
    // 클라이언트에게 메시지를 보내는 메서드 추가
    fn send_message(&self, message: &str) -> io::Result<()> {
        let mut stream = self.stream.try_clone()?;
        stream.write_all(message.as_bytes())?;
        stream.flush()?;
        Ok(())
    }
}

// 모든 클라이언트에게 메시지를 브로드캐스트하는 함수
fn broadcast_to_all(
    clients: &HashMap<String, Client>,
    message: &str,
    except: Option<&str>
) {
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

// Process incoming messages from clients
fn handle_client(
    stream: TcpStream,
    nickname: String,
    clients: Arc<Mutex<HashMap<String, Client>>>,
) -> io::Result<()> {
    let client_addr = stream.peer_addr()?;
    
    // 클라이언트 접속 정보를 서버 콘솔에 출력
    {
        let num_users = clients.lock().unwrap().len();
        println!("{} joined from {}:{}. There are {} users in the room", 
                 nickname, client_addr.ip(), client_addr.port(), num_users);
    }

    // 새 클라이언트에게 환영 메시지 전송
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

    // 다른 모든 클라이언트에게 새 유저 접속 메시지 브로드캐스트
    {
        let clients_lock = clients.lock().unwrap();
        let num_users = clients_lock.len();
        let join_msg = format!(
            "[{} joined from {}:{}. There are {} users in the room.]\n",
            nickname, client_addr.ip(), client_addr.port(), num_users
        );
        
        broadcast_to_all(&clients_lock, &join_msg, Some(&nickname));
    }

    // 클라이언트에서 메시지 수신하기 위한 리더 설정
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut buffer = Vec::new();

    // 메인 메시지 처리 루프
    loop {
        buffer.clear();
        let bytes_read = reader.read_until(b'\n', &mut buffer)?;
        
        // 연결 종료 확인
        if bytes_read == 0 {
            break;
        }

        // 버퍼가 비어있거나 너무 짧은 경우 스킵
        if buffer.is_empty() || bytes_read <= 1 {
            continue;
        }

        // 첫 바이트는 명령 코드
        let cmd = buffer[0];
        let content = String::from_utf8_lossy(&buffer[1..bytes_read-1]).to_string();
        
        // 디버깅: 수신된 명령과 내용 출력
        println!("Received from {}: cmd={}, content='{}', bytes={}", 
                 nickname, cmd, content, bytes_read);

        match cmd {
            CMD_CHAT => {
                // "I hate professor" 검사
                if content.to_lowercase() == "i hate professor" {
                    // 우선 해당 클라이언트에게만 알림 메시지 전송 (명확한 메시지)
                    let mut banned_stream = stream.try_clone()?;
                    banned_stream.write_all("You sent a prohibited message and will be disconnected.\n".as_bytes())?;
                    banned_stream.flush()?;
                    
                    // 다른 클라이언트들에게 알림 (해당 클라이언트 제외)
                    {
                        let mut clients_lock = clients.lock().unwrap();
                        let num_remaining = clients_lock.len() - 1;
                        
                        // 메시지 준비 - "disconnected" 단어 사용 안함
                        let notify_msg = format!(
                            "[{} was removed for prohibited message. {} users remain.]\n",
                            nickname, num_remaining
                        );
                        
                        // 다른 클라이언트들에게만 전송 (해당 클라이언트는 제외)
                        for (nick, client) in clients_lock.iter() {
                            if *nick != nickname {
                                let _ = client.send_message(&notify_msg);
                            }
                        }
                        
                        // 클라이언트 목록에서 제거
                        clients_lock.remove(&nickname);
                        
                        println!("{} is removed for sending prohibited message. There are {} users now", 
                                nickname, num_remaining);
                    }
                    
                    // 이 클라이언트의 메시지 처리 종료
                    return Ok(());
                }
                
                // 서버 콘솔에 메시지 출력
                println!("{}: {}", nickname, content);
                
                // 다른 모든 클라이언트에게 채팅 브로드캐스트
                {
                    let clients_lock = clients.lock().unwrap();
                    let msg = format!("{}> {}\n", nickname, content);
                    broadcast_to_all(&clients_lock, &msg, Some(&nickname));
                }
            }
            CMD_LIST => {
                let clients_lock = clients.lock().unwrap();
                let mut list_msg = String::new();
                
                // 목록 메시지 포맷팅
                list_msg.push_str("Connected users:\n");
                for (nick, client) in clients_lock.iter() {
                    list_msg.push_str(&format!("{}, {}, {}\n", nick, client.ip, client.port));
                }
                
                // 요청한 클라이언트에게만 목록 전송
                if let Some(client) = clients_lock.get(&nickname) {
                    let _ = client.send_message(&list_msg);
                }
            }
            CMD_TO => {
                // 형식: nickname message
                if let Some(space_idx) = content.find(' ') {
                    let target = &content[..space_idx];
                    let message = &content[space_idx + 1..];
                    
                    let clients_lock = clients.lock().unwrap();
                    
                    if clients_lock.contains_key(target) {
                        // 비밀 메시지 전송
                        if let Some(client) = clients_lock.get(target) {
                            let msg = format!("from: {}> {}\n", nickname, message);
                            let _ = client.send_message(&msg);
                        }
                    } else {
                        // 대상이 존재하지 않는 경우 오류 메시지
                        if let Some(client) = clients_lock.get(&nickname) {
                            let error_msg = format!("Error: User '{}' does not exist.\n", target);
                            let _ = client.send_message(&error_msg);
                        }
                    }
                }
            }
            CMD_EXCEPT => {
                // 형식: nickname message
                if let Some(space_idx) = content.find(' ') {
                    let except_nick = &content[..space_idx];
                    let message = &content[space_idx + 1..];
                    
                    let clients_lock = clients.lock().unwrap();
                    
                    if clients_lock.contains_key(except_nick) {
                        // 지정된 유저와 발신자를 제외한 모든 유저에게 전송
                        for (nick, client) in clients_lock.iter() {
                            if *nick != nickname && *nick != except_nick {
                                let msg = format!("{}> {}\n", nickname, message);
                                let _ = client.send_message(&msg);
                            }
                        }
                    } else {
                        // 대상이 존재하지 않는 경우 오류 메시지
                        if let Some(client) = clients_lock.get(&nickname) {
                            let error_msg = format!("Error: User '{}' does not exist.\n", except_nick);
                            let _ = client.send_message(&error_msg);
                        }
                    }
                }
            }
            CMD_BAN => {
                let ban_nick = content.trim();
                let mut clients_lock = clients.lock().unwrap();
                
                if ban_nick != nickname && clients_lock.contains_key(ban_nick) {
                    // 차단 메시지 전송
                    if let Some(client) = clients_lock.get(ban_nick) {
                        let ban_msg = format!("you are banned by {}\n", nickname);
                        let _ = client.send_message(&ban_msg);
                    }
                    
                    // 클라이언트 제거
                    clients_lock.remove(ban_nick);
                    
                    // 차단 메시지 브로드캐스트
                    let ban_broadcast = format!(
                        "[{} left the room. There are {} users now]\n",
                        ban_nick, clients_lock.len()
                    );
                    
                    println!("{} was banned by {}. There are {} users now", 
                             ban_nick, nickname, clients_lock.len());
                    
                    broadcast_to_all(&clients_lock, &ban_broadcast, None);
                } else if ban_nick == nickname {
                    // 자신을 차단할 수 없음
                    if let Some(client) = clients_lock.get(&nickname) {
                        let error_msg = "Error: You cannot ban yourself.\n";
                        let _ = client.send_message(error_msg);
                    }
                } else {
                    // 유저가 존재하지 않음
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
                    // 핑 응답 직접 전송
                    let rtt_msg = format!("RTT: {:?}\n", start.elapsed());
                    let _ = client.send_message(&rtt_msg);
                }
            }
            CMD_EXIT => {
                // 클라이언트 종료
                let mut clients_lock = clients.lock().unwrap();
                clients_lock.remove(&nickname);
                
                let exit_msg = format!(
                    "[{} left the room. There are {} users now]\n",
                    nickname, clients_lock.len()
                );
                
                println!("{} left the room. There are {} users now", nickname, clients_lock.len());
                
                // 종료 메시지 브로드캐스트
                broadcast_to_all(&clients_lock, &exit_msg, None);
                
                break;
            }
            _ => {
                println!("Invalid command: {}", cmd);
            }
        }
    }

    // 연결 해제 시 클라이언트 제거
    let mut clients_lock = clients.lock().unwrap();
    clients_lock.remove(&nickname);
    
    println!("{} disconnected. There are {} users now", nickname, clients_lock.len());
    
    let leave_msg = format!(
        "[{} left the room. There are {} users now]\n",
        nickname, clients_lock.len()
    );
    
    // 퇴장 메시지 브로드캐스트
    broadcast_to_all(&clients_lock, &leave_msg, None);

    Ok(())
}

fn main() -> io::Result<()> {
    // 지정된 포트에 TCP 리스너 생성
    let listener = TcpListener::bind(format!("0.0.0.0:{}", PORT))?;
    println!("Server listening on port {}", PORT);

    // 연결된 클라이언트 저장
    let clients: Arc<Mutex<HashMap<String, Client>>> = Arc::new(Mutex::new(HashMap::new()));

    // 들어오는 연결 수락
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let client_addr = stream.peer_addr()?;
                println!("New connection from {}:{}", client_addr.ip(), client_addr.port());

                // 최대 클라이언트 수 확인
                let current_clients = clients.lock().unwrap().len();
                if current_clients >= MAX_CLIENTS {
                    let error_msg = "chatting room full. cannot connect\n";
                    let mut stream_clone = stream.try_clone()?;
                    stream_clone.write_all(error_msg.as_bytes())?;
                    stream_clone.flush()?;
                    continue;
                }

                // 클라이언트에서 닉네임 읽기
                let mut reader = BufReader::new(&stream);
                let mut nickname = String::new();
                reader.read_line(&mut nickname)?;
                nickname = nickname.trim().to_string();

                // 닉네임 길이 및 문자 제한 확인
                if nickname.len() > 10 || nickname.contains(|c: char| !c.is_ascii_alphanumeric()) {
                    let error_msg = "nickname must be <= 10 characters, English only, no spaces or special chars\n";
                    let mut stream_clone = stream.try_clone()?;
                    stream_clone.write_all(error_msg.as_bytes())?;
                    stream_clone.flush()?;
                    continue;
                }

                // 중복 닉네임 확인
                let mut clients_lock = clients.lock().unwrap();
                if clients_lock.contains_key(&nickname) {
                    let error_msg = "nickname already used by another user. cannot connect\n";
                    let mut stream_clone = stream.try_clone()?;
                    stream_clone.write_all(error_msg.as_bytes())?;
                    stream_clone.flush()?;
                    continue;
                }

                // 클라이언트 목록에 추가
                let client = Client::new(nickname.clone(), stream.try_clone()?);
                clients_lock.insert(nickname.clone(), client);
                drop(clients_lock);

                // 클라이언트 핸들링할 새 스레드 생성
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