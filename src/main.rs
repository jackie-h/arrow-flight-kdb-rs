use arrow_flight::flight_service_server::FlightServiceServer;
use tonic::transport::Server;
use crate::server::FlightServiceImpl;

pub mod server;


#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:9001".parse()?;
    //let addr = "[::1]:50051".parse()?;
    let service = FlightServiceImpl {};

    let svc = FlightServiceServer::new(service);

    Server::builder().add_service(svc).serve(addr).await?;

    Ok(())
}
