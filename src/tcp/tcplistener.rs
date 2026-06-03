use std::net::{TcpListener,TcpStream};

fn handle_client(stream : TcpStream){
  //i need to make it more crisp and need to make it more well and more 
}

fn main() -> std::io::Result<()>{
    let listener = TcpListener.bind("127.0.0.1:80").unwrap();

    for stream in listener.incoming() {
        handle_client(stream?);
    }
}
