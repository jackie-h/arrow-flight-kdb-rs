#[cfg(test)]
mod tests {
    use std::time::Duration;

    use arrow_flight::{FlightClient, FlightDescriptor};
    use arrow_flight::error::FlightError;
    use arrow_flight::flight_service_server::FlightServiceServer;
    use tokio::time::sleep;
    use tonic::Code;
    use tonic::transport::{Channel, Server, Uri};

    use arrow_flight_kdb_rs::server::FlightServiceImpl;

    #[tokio::test]
    async fn test_get_flight_info() {

        let addr = "127.0.0.1:9001".parse().unwrap();

        //NOTE MUST BE STARTED IN A SEPARATE THREAD
        tokio::spawn(async move {
            let service = FlightServiceImpl {};

            let svc = FlightServiceServer::new(service);

            Server::builder().add_service(svc).serve(addr).await.unwrap();
        });

        sleep(Duration::from_secs(2)).await;


        //Client
        let url = format!("http://{}", addr);
        let uri: Uri = url.parse().expect("Valid URI");
        let channel = Channel::builder(uri)
            .timeout(Duration::from_secs(2))
            .connect()
            .await
            .expect("error connecting to server");

        let mut client = FlightClient::new(channel);
        client.add_header("foo-header", "bar-header-value").unwrap();
        let request = FlightDescriptor::new_cmd(b"My Command".to_vec());

        let result = client.get_flight_info(request).await;

        let err = result.err().unwrap();
        match err {
            FlightError::Tonic(s) => assert_eq!(s.code(), Code::Unimplemented),
            _ => panic!("Fail")
        }

    }

}
