# KERES Engine Server API Documentation

This document describes the binary API endpoints for the KERES Engine server. All endpoints use raw binary formats for performance and minimal network overhead. This guide is intended for client developers.

## General Notes
- All requests and responses use the `application/octet-stream` content type.
- Board data is always sent and received in the KERES binary format (see piece_encoding.instructions.md for details).
- Moves are represented as `u16` values in little-endian byte order.

---

## Endpoints

### 1. `GET /new`
**Description:**
Returns a new game board in binary format.

**Response:**
- Status: `200 OK`
- Body: `[u8; BOARD_SIZE + 1]` (binary board data)

---

### 2. `POST /moves`
**Description:**
Returns all possible moves for a given board.

**Request:**
- Body: `[u8; BOARD_SIZE + 1]` (binary board data)

**Response:**
- Status: `200 OK`
- Body: Concatenated list of possible moves, each as a `u16` in little-endian format:
  - `[u16, u16, ...]` (binary)

---

### 3. `POST /play`
**Description:**
Applies a move to a board and returns the new board state.

**Request:**
- Body: `[u8; BOARD_SIZE + 1]` (binary board data) followed by `[u16]` (move to play, little-endian)
  - Total length: `BOARD_SIZE + 3` bytes

**Response:**
- Status: `200 OK`
- Body: `[u8; BOARD_SIZE + 1]` (new binary board data)

---

## Binary Format Details
- See `.github/instructions/piece_encoding.instructions.md` for board encoding rules.
- Moves are encoded as `u16` values. Use the same encoding as the engine's move representation.

## Error Handling
- If the request body is malformed or the board data is invalid, the server responds with `400 Bad Request`.
- If an internal error occurs, the server responds with `500 Internal Server Error`.

## Example Usage
- To get all possible moves:
  1. Send a POST to `/moves` with the board binary.
  2. Parse the response as a sequence of `u16` values.
- To play a move:
  1. Concatenate the board binary and the move (`u16` LE).
  2. Send a POST to `/play`.
  3. The response is the new board binary.

---

For board and move encoding details, refer to the engine documentation and piece encoding instructions.
