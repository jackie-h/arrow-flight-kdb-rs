#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::net::SocketAddr;
    use std::time::Duration;
    use arrow_flight::error::FlightError;
    use arrow_flight::{FlightClient, FlightDescriptor};
    use arrow_flight::flight_service_server::FlightServiceServer;
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;
    use tonic::Status;
    use tonic::transport::{Channel, Server, Uri};

    // async fn setup() -> mongodb::Database {
    //     tokio_test::block_on(async {
    //         let client_uri = "mongodb://127.0.0.1:27017";
    //         let options = ClientOptions::parse(&client_uri).await;
    //         let client_result = Client::with_options(options.unwrap());
    //         let client = client_result.unwrap();
    //         client.database("my_database")
    //     })
    // }
    #[tokio::test]
    async fn test_get_flight_info() {

        do_test(|test_server, mut client| async move {
            client.add_header("foo-header", "bar-header-value").unwrap();
            let request = FlightDescriptor::new_cmd(b"My Command".to_vec());

            let expected_response = test_flight_info(&request);
            test_server.set_get_flight_info_response(Ok(expected_response.clone()));

            let response = client.get_flight_info(request.clone()).await.unwrap();

            assert_eq!(response, expected_response);
            assert_eq!(test_server.take_get_flight_info_request(), Some(request));
            ensure_metadata(&client, &test_server);
        })
            .await;
    }

    /// Runs the future returned by the function,  passing it a test server and client
    async fn do_test<F, Fut>(f: F)
        where
            F: Fn(FlightServer, FlightClient) -> Fut,
            Fut: Future<Output = ()>,
    {
        let addr = "[::1]:50051".parse()?;
        let service = FlightServiceImpl {};

        let svc = FlightServiceServer::new(service);

        Server::builder().add_service(svc).serve(addr).await?;
        let fixture = TestFixture::new(&test_server).await;
        let client = FlightClient::new(fixture.channel().await);

        // run the test function
        f(test_server, client).await;

        // cleanly shutdown the test fixture
        fixture.shutdown_and_wait().await
    }

    fn expect_status(error: FlightError, expected: Status) {
        let status = if let FlightError::Tonic(status) = error {
            status
        } else {
            panic!("Expected FlightError::Tonic, got: {error:?}");
        };

        assert_eq!(
            status.code(),
            expected.code(),
            "Got {status:?} want {expected:?}"
        );
        assert_eq!(
            status.message(),
            expected.message(),
            "Got {status:?} want {expected:?}"
        );
        assert_eq!(
            status.details(),
            expected.details(),
            "Got {status:?} want {expected:?}"
        );
    }

    /// Creates and manages a running TestServer with a background task
    struct TestFixture {
        /// channel to send shutdown command
        shutdown: Option<tokio::sync::oneshot::Sender<()>>,

        /// Address the server is listening on
        addr: SocketAddr,

        // handle for the server task
        handle: Option<JoinHandle<Result<(), tonic::transport::Error>>>,
    }

    impl TestFixture {
        /// create a new test fixture from the server
        pub async fn new(test_server: &TestFlightServer) -> Self {
            // let OS choose a a free port
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();

            println!("Listening on {addr}");

            // prepare the shutdown channel
            let (tx, rx) = tokio::sync::oneshot::channel();

            let server_timeout = Duration::from_secs(DEFAULT_TIMEOUT_SECONDS);

            let shutdown_future = async move {
                rx.await.ok();
            };

            let serve_future = tonic::transport::Server::builder()
                .timeout(server_timeout)
                .add_service(test_server.service())
                .serve_with_incoming_shutdown(
                    tokio_stream::wrappers::TcpListenerStream::new(listener),
                    shutdown_future,
                );

            // Run the server in its own background task
            let handle = tokio::task::spawn(serve_future);

            Self {
                shutdown: Some(tx),
                addr,
                handle: Some(handle),
            }
        }

        /// Return a [`Channel`] connected to the TestServer
        pub async fn channel(&self) -> Channel {
            let url = format!("http://{}", self.addr);
            let uri: Uri = url.parse().expect("Valid URI");
            Channel::builder(uri)
                .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECONDS))
                .connect()
                .await
                .expect("error connecting to server")
        }

        /// Stops the test server and waits for the server to shutdown
        pub async fn shutdown_and_wait(mut self) {
            if let Some(shutdown) = self.shutdown.take() {
                shutdown.send(()).expect("server quit early");
            }
            if let Some(handle) = self.handle.take() {
                println!("Waiting on server to finish");
                handle
                    .await
                    .expect("task join error (panic?)")
                    .expect("Server Error found at shutdown");
            }
        }
    }

    impl Drop for TestFixture {
        fn drop(&mut self) {
            if let Some(shutdown) = self.shutdown.take() {
                shutdown.send(()).ok();
            }
            if self.handle.is_some() {
                // tests should properly clean up TestFixture
                println!("TestFixture::Drop called prior to `shutdown_and_wait`");
            }
        }
    }
}
