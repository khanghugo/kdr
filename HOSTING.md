# Hosting kdr on the web

## 1. Overview

Main components:

* Client: WASM module that renders demos/maps in-browser
* Backend: Resource server distributing maps/replays
* WebSocket Server (Optional): For live feed from a server

kdr repo provides client module, backend documentation and implementation, and [WebSocket documentation](./puppeteer/README.md).

---

## 2. Hosting API Server

Check [rest-api-server](./rest-api-server/) module in this repo for the API server implementation.

TODO: add the api documentation

### 2.1 Server Configuration (`config.toml`)

API server reads its configuration from a `config.toml` file. By default, the server will look for `config.toml` in the same directory as the server binary. Alternatively, you can specify a custom path to the configuration file using the environment variable `KDR_API_CONFIG_PATH`.

**Default `config.toml` Structure and Explanation:**

Check [dist/server.toml](./dist/server.toml) for lots of comments

**Understanding Resource Distribution (`use_resmake_zip`):**

The KDR API Server has two primary methods for distributing map files:

* **Native Way (`use_resmake_zip = false`):**
  * **How it works:** When a client requests a map (e.g., `de_dust2.bsp`), the server reads the `.bsp` file to determine all required assets such as `.mdl` models, `.spr` sprites, `.tga` skyboxes, `.wav` sounds, etc. It then locates these individual files within the `game_dir` and its subdirectories, packs them into a ZIP archive on the fly, and sends it to the client. In the case that `game_dir` does not match, it has to read all of hardcoded "common game mods". This process also applies to replays: when a replay is requested, the server first extracts the map name from the replay file and then proceeds to find and zip the necessary map-related files using this "native" method.
    * **Pros:** Requires no pre-processing of map files, results in less storage used.
    * **Cons:** Can be resource-intensive (CPU/disk I/O) on the server, especially with many concurrent requests or large maps, as it involves real-time file scanning and zipping. In the case where the map uses external texture, the server must read all WAD files to snipe for that one texture.

* **Gchimp Way (`use_resmake_zip = true`):**
  * **How it works:** This method leverages an external tool called `gchimp resmake`. Before running API server, the host uses `gchimp remake` to pre-process all maps in a game mod. `gchimp resmake` takes a `.bsp` file and bundles it with all its dependencies into a single, pre-made `.zip` archive (e.g., `de_dust2.zip` for `de_dust2.bsp`). These `.zip` archives should be placed in the same directory as their corresponding `.bsp` files. When a client requests a map, server simply locates and sends this pre-generated `.zip` file, without needing to perform any real-time dependency scanning or zipping.
  * **Recommended `gchimp ResMake` command:**

    ```bash
    resmake -f /pat/to/hl.exe/<game mod>/maps --wad-check --include-default --skip-created-res
    ```

    This command is included inside the default config file.

    * **Pros:** Significantly reduces server load during requests, as map packing is done offline. Faster response times for clients.
    * **Cons:** Extra storage usage

### 2.2 Running the Server

To start kdr API server, simply execute its binary. Ensure the `config.toml` file (or the path specified by `KDR_API_CONFIG_PATH`) is correctly set up.

```bash
./kdr-api-server # Or the name of your compiled binary
```

---

## 3. Hosting Web Client

kdr web client is a WASM module that must be served by a standard web server (e.g., Apache, Nginx, or any static file server). You will typically place the `kdr.wasm` file and its accompanying `kdr.js` loader, along with an `index.html` file, in a web-accessible directory.

**Example `index.html` (minimal):**

Check [www/index.html](./www/index.html)

---

## 4. Controlling the Client via URL Queries

Web client can be controlled by URL query parameters, allowing hosts to direct users to specific map or replay without fiddling inside the app.

**TODO:** ADD SOMETHING

---

## 5. Live Replay Feed with WebSocket Server

Beyond static content serving, kdr web client supports real-time live replay feeds via a WebSocket connection. This allows the host to dictate the client's current view (origin, angle), change maps, and effectively provide a "live" viewing experience.

**Enabling Live Feed:**

WebSocket server is a different implementation from the API server. The current API server does not contain WebSocket implementation. But, there is [puppeteer-ws-mock-server](./puppeteer-ws-mock-server/) that provides a basic WebSocket connection that you can try out.

To enable the WebSocket connection, add it inside the [www/index.html](./www/index.html) file. Take a read and you will see it.

Upon the client start up, WS connection will be established.

**WebSocket Communication Protocol:**

Check [puppeteer/README.md](./puppeteer/README.md)

---

## 6. API Server Endpoints (Reference)

TODO

---
