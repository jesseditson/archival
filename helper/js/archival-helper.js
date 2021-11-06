/**
 * When running archival in development mode (archival run), this file is
 * injected into all pages, and is responsible for reloading the page when the
 * source has changed.
 */

(function () {
  const remotePort = $PORT;
  const CONNECTING_COLOR = "#bd270d";
  const CONNECTED_COLOR = "#19bd0d";
  const CHECK_INTERVAL = 500;
  const DISCONNECTED_INTERVAL = 1000;
  const connectionDot = document.createElement("div");
  connectionDot.style = `position: absolute; z-index: 9999; bottom: 10px; right: 10px; background-color: ${CONNECTING_COLOR}; width: 15px; height: 15px; border-radius: 50%; opacity: 0.8;`;
  connectionDot.setAttribute("title", "Archival Dev Server: Connecting");
  connectionDot.addEventListener(
    "mouseenter",
    () => (connectionDot.style.opacity = 0.2)
  );
  connectionDot.addEventListener(
    "mouseleave",
    () => (connectionDot.style.opacity = 0.8)
  );

  let lastContact = -1;
  let isConnecting = false;
  let connection;

  function connectionLoop() {
    connection.send(`page:${window.location.pathname}`);
    if (Date.now() - lastContact > DISCONNECTED_INTERVAL) {
      setConnected(false);
      connectSocket();
    }
    setTimeout(connectionLoop, CHECK_INTERVAL);
  }

  function setConnected(connected) {
    connectionDot.style.backgroundColor = connected
      ? CONNECTED_COLOR
      : CONNECTING_COLOR;
    connectionDot.setAttribute(
      "title",
      `Archival Dev Server: ${connected ? "Connected" : "Disconnected"}`
    );
  }

  window.onload = () => {
    connectSocket(true);
  };

  function connectSocket(init) {
    if (isConnecting) {
      return;
    }
    isConnecting = true;
    console.log(
      `${init ? "connecting" : "reconnecting"} to archival dev server...`
    );
    document.body.appendChild(connectionDot);
    connection = new WebSocket(`ws://localhost:${remotePort}`);
    connection.onerror = () => {
      isConnecting = false;
    };

    connection.onopen = () => {
      isConnecting = false;
      connection.send("connected");
      if (init) {
        connectionLoop();
      }
    };
    connection.onmessage = (event) => {
      lastContact = Date.now();
      switch (event.data) {
        case "ready":
          console.log("connected to archival dev server.");
          break;
        case "ok":
          setConnected(true);
          break;
        case "refresh":
          window.location.reload();
          break;
        default:
          console.log(`receieved unexpected message ${event.data}`);
          break;
      }
    };
  }
})();
