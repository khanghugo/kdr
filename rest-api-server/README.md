# kdr Web REST API Server

kdr Web model goes:

0. Client connects to a server serving static page. The client receives WASM kdr.

1. Client parses demo to get map name and send it to server.

2. Server receives map name then gathers data related to the map and send all back to client.

3. Client receives map resources and plays back the replay.

This is the REST API process responsible for gathering data related to a map and responds to the client requests.
