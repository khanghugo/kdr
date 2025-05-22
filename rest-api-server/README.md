# kdr Web REST API Server

kdr Web model goes:

0. Client connects to a server serving static page. The client receives WASM kdr.

1. Client parses demo to get map name and send it to server.

2. Server receives map name then gathers data related to the map and send all back to client.

3. Client receives map resources and plays back the replay.

This is the REST API process responsible for gathering data related to a map and responds to the client requests.

## KDR API Server Specification (v1)

This document outlines the REST API endpoints provided by the KDR API Server, specifically for version 1 of the API.

### Base URL

All endpoints are relative to the server's root URL and are prefixed with `/v1/` (e.g., `http://<server_ip>:<port>/v1`).

---

### Data Structures

#### `MapList`

Represents a list of available maps, grouped by game modification.

```json
{
  "cstrike": [
    "de_dust2",
    "cs_assault"
  ],
  "hl": [
    "crossfire"
  ]
}
```

* **Type:** `HashMap<String, HashSet<String>>` (JSON object)
  * Keys: `string` (Game modification folder name, e.g., "cstrike", "hl").
  * Values: `array of string` (List of map names within that game mod, e.g., "de_dust2").

#### `ReplayList`

Represents a list of available replay files.

```json
[
  "replay_001.dem",
  "game_session_123.dem"
]
```

* **Type:** `Vec<String>` (JSON array of strings)
  * Elements: `string` (The full name of a replay file).

---

### Endpoints

All listed paths are relative to the `/v1` prefix.

#### 1. `GET /v1/common-resource`

**Description:** Retrieves a common resource file, typically a `.zip` archive, needed by the client application. These are shared assets not specific to any single map or replay. This usually includes weapon sound, movement sound, view models, and player models.

**Request:**

* **Method:** `GET`
* **Path:** `/v1/common-resource`
* **Headers:** None
* **Body:** None

**Responses:**

* **`200 OK`**:
  * **Content-Type:** `application/zip`
  * **Content-Transfer-Encoding:** `binary`
  * **Content-Length:** (length of the ZIP file in bytes)
  * **Content-Disposition:** `attachment; filename="common.zip"`
  * **Body:** Binary content of the `common.zip` file.
* **`204 No Content`**: If no common resource is configured or available on the server.

---

#### 2. `GET /v1/maps/{game_mod}/{map_name}`

**Description:** Requests a specific map resource. The server will respond with a `.zip` archive containing the map file (`.bsp`) and all its associated files.

**Request:**

* **Method:** `GET`
* **Path:** `/v1/maps/{game_mod}/{map_name}`
* **Path Parameters:**
  * `game_mod` (string, **required**): The name of the game modification folder (e.g., "cstrike", "valve").
  * `map_name` (string, **required**): The name of the map without the `.bsp` extension (e.g., "de_dust2", "crossfire").
* **Example URL:** `http://<server_ip>:<port>/v1/maps/cstrike/de_dust2`
* **Headers:** None
* **Body:** None

**Responses:**

* **`200 OK`**:
  * **Content-Type:** `application/zip`
  * **Content-Transfer-Encoding:** `binary`
  * **Content-Length:** (length of the ZIP file in bytes)
  * **Content-Disposition:** `attachment; filename="<sanitized_map_name>.zip"`
  * **Body:** Binary content of the requested map's ZIP file.
* **`404 Not Found`**: If the requested map cannot be found or bad request.
  * **Body:** A plain text string: "Cannot find the requested map."

---

#### 3. `GET /v1/replays/{replay_name}`

**Description:** Requests a specific replay file. The replay data is returned as a MessagePack serialized binary blob.

**Request:**

* **Method:** `GET`
* **Path:** `/v1/replays/{replay_name}`
* **Path Parameters:**
  * `replay_name` (string, **required**): The full name of the replay file, including extension (e.g., "my_awesome_replay.dem").
* **Example URL:** `http://<server_ip>:<port>/v1/replays/my_awesome_replay.dem`
* **Headers:** None
* **Body:** None

**Responses:**

* **`200 OK`**:
  * **Content-Type:** `application/x-msgpack` (or `application/octet-stream` if `x-msgpack` isn't supported, with content understood to be MessagePack)
  * **Body:** Binary content of the MessagePack serialized replay blob.
* **`404 Not Found`**: If the requested replay cannot be found.
  * **Body:** A plain text string: "Cannot find the requested replay."

---

#### 4. `GET /v1/maps`

**Description:** Retrieves a list of all available maps, categorized by their respective game modifications.

**Request:**

* **Method:** `GET`
* **Path:** `/v1/maps`
* **Headers:** None
* **Body:** None

**Responses:**

* **`200 OK`**:
  * **Content-Type:** `application/json`
  * **Body:** A JSON object representing the `MapList` structure.

---

#### 5. `GET /v1/replays`

**Description:** Retrieves a list of all available replay files.

**Request:**

* **Method:** `GET`
* **Path:** `/v1/replays`
* **Headers:** None
* **Body:** None

**Responses:**

* **`200 OK`**:
  * **Content-Type:** `application/json`
  * **Body:** A JSON array of strings, representing the `ReplayList` structure.

---

#### 6. `POST /v1/update-map-list`

**Description:** Triggers an update of the server's internal map list by re-scanning the configured `game_dir`. This endpoint requires a secret for authorization.

**Request:**

* **Method:** `POST`
* **Path:** `/v1/update-map-list`
* **Content-Type:** `application/json`
* **Body:** A JSON object containing the secret.

    ```json
    {
    "secret": "your_configured_secret_key"
    }
    ```

  * `secret` (string, **required**): The secret key configured on the server to authorize this action. This should be transmitted securely over HTTPS.

**Responses:**

* **`200 OK`**: If the map list was successfully updated. (No body)
* **`403 Forbidden`**: If the provided `secret` does not match the server's configured secret. (No body)
* **`500 Internal Server Error`**: If there was an error updating the map list on the server. (No body)

---

#### 7. `POST /v1/update-replay-list`

**Description:** Triggers an update of the server's internal replay list by re-scanning the configured `replay_folders`. This endpoint requires a secret for authorization.

**Request:**

* **Method:** `POST`
* **Path:** `/v1/update-replay-list`
* **Content-Type:** `application/json`
* **Body:** A JSON object containing the secret.

    ```json
    {
      "secret": "your_configured_secret_key"
    }
    ```

  * `secret` (string, **required**): The secret key configured on the server to authorize this action. This should be transmitted securely over HTTPS.

**Responses:**

* **`200 OK`**: If the replay list was successfully updated. (No body)
* **`403 Forbidden`**: If the provided `secret` does not match the server's configured secret. (No body)
* **`500 Internal Server Error`**: If there was an error updating the replay list on the server. (No body)

---

#### 8. `GET /v1/health`

**Description:** A simple health check endpoint to verify that the server is running and responsive.

**Request:**

* **Method:** `GET`
* **Path:** `/v1/health`
* **Headers:** None
* **Body:** None

**Responses:**

* **`200 OK`**: If the server is operational. (No body)
