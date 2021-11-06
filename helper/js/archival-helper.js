/**
 * When running archival in development mode (archival run), this file is
 * injected into all pages, and is responsible for reloading the page when the
 * source has changed.
 */

window.onload = () => {
    connectSocket($PORT);
};

function connectSocket(remotePort) {
    console.log("connecting...")
  navigator.tcpPermission
    .requestPermission({ remoteAddress: "localhost", remotePort })
    .then(() => {
        console.log("connected.")
      // Permission was granted
      // Create a new TCP client socket and connect to remote host
      const mySocket = new TCPSocket("localhost", remotePort);
      console.log(mySocket)

      mySocket.readable
        .getReader()
        .read()
        .then(({ value, done }) => {
          if (!done) {
            // Response received, log it:
            console.log("Data received from server:" + value);
          }

          // Close the TCP connection
          mySocket.close();
        });
      // Send data to server
      mySocket.writeable.write("Hello World").then(
        () => {
          // Data sent sucessfully, wait for response
          console.log("Data has been sent to server");
        },
        (e) => console.error("Sending error: ", e)
      );
    });
}
