# Mercure Real-time Game Updates Implementation

## Overview

This implementation adds real-time game updates using Mercure for the Keres game platform. When playing against AI or remote opponents, game state changes are broadcast to all connected clients via Server-Sent Events (SSE).

## Architecture

### Backend Components

#### 1. Message Queue (Symfony Messenger)
- **ProcessAiMoveMessage**: Message containing the game UUID to process
- **ProcessAiMoveHandler**: Handles AI move calculation asynchronously
  - Queries the Rust engine for the next AI move
  - Updates game state in database
  - Publishes update to Mercure

#### 2. Mercure Publisher
- **GameUpdatePublisher**: Service that publishes game updates to Mercure hub
  - Formats game state as JSON with timestamp
  - Publishes to topic `game/{uuid}`
  - Includes timestamp in microseconds for out-of-order detection

#### 3. Action Controller Updates
- **SubmitMoveAction**: Updated to handle different game modes
  - **Hot Seat Mode**: Returns response synchronously (no real-time needed)
  - **AI Mode**: Returns response immediately, dispatches async message for AI move
  - **Multiplayer Mode**: Ready for future implementation

### Frontend Components

#### 1. MercureClient
- Manages EventSource connection to Mercure hub
- Subscribes to game-specific topic: `game/{uuid}`
- Handles incoming updates with timestamp validation
- Ignores out-of-order updates (timestamp checking)

#### 2. GameController Integration
- `initializeMercure(gameUuid)`: Connects to Mercure for a specific game
- `handleMercureUpdate(update)`: Processes incoming game state updates
- Automatically updates board, moves, and UI when updates arrive

#### 3. Application Integration
- Mercure URL exposed via meta tag in base template
- Auto-connects for AI games on page load
- Shows "Waiting for AI..." status when move is processing

## Flow Diagram

### AI Game Move Flow

```
1. Player makes move
   ↓
2. POST /play/{uuid}/move
   ↓
3. Save player move to DB
   ↓
4. Return response immediately
   ↓
5. Dispatch ProcessAiMoveMessage (async)
   ↓
6. [Background] ProcessAiMoveHandler:
   - Query Rust engine for AI move
   - Save AI move to DB
   - Publish to Mercure
   ↓
7. Client receives SSE update
   ↓
8. Update board state in real-time
```

## Configuration

### Environment Variables (compose.yaml)
```yaml
MERCURE_URL: http://php/.well-known/mercure
MERCURE_PUBLIC_URL: https://localhost/.well-known/mercure
MERCURE_JWT_SECRET: !ChangeThisMercureHubJWTSecretKey!
```

### Messenger Transport (messenger.yaml)
```yaml
framework:
    messenger:
        transports:
            async: '%env(MESSENGER_TRANSPORT_DSN)%'
        routing:
            'App\Message\ProcessAiMoveMessage': async
```

### Mercure Configuration (mercure.yaml)
```yaml
mercure:
    hubs:
        default:
            url: '%env(MERCURE_URL)%'
            public_url: '%env(MERCURE_PUBLIC_URL)%'
            jwt:
                secret: '%env(MERCURE_JWT_SECRET)%'
                publish: '*'
```

## Data Format

### Game Update Message
```json
{
  "success": true,
  "board": "base64_encoded_board_state",
  "moves": "base64_encoded_moves_list",
  "gameOver": false,
  "whiteWins": false,
  "draw": false,
  "timestamp": 1738668000000000
}
```

### Timestamp Handling
- Timestamps are in microseconds (μs) since Unix epoch
- Each update includes a timestamp
- Client tracks last received timestamp
- Updates with older timestamps are ignored
- This prevents race conditions and out-of-order delivery

## Database Schema

### Messenger Messages Table
```sql
CREATE TABLE messenger_messages (
    id BIGSERIAL NOT NULL,
    body TEXT NOT NULL,
    headers TEXT NOT NULL,
    queue_name VARCHAR(190) NOT NULL,
    created_at TIMESTAMP(0) WITHOUT TIME ZONE NOT NULL,
    available_at TIMESTAMP(0) WITHOUT TIME ZONE NOT NULL,
    delivered_at TIMESTAMP(0) WITHOUT TIME ZONE DEFAULT NULL,
    PRIMARY KEY(id)
);
```

## Testing

### Manual Testing Steps
1. Start the application with Docker Compose
2. Create a new AI game
3. Make a move as the player
4. Observe "Waiting for AI..." status
5. AI move should appear automatically without refresh
6. Verify move history updates correctly

### Checking Mercure Connection
- Open browser DevTools → Network tab
- Filter by "EventSource" or look for `.well-known/mercure`
- Should see persistent connection with EventSource type
- Updates appear as messages in the connection

### Debugging
- Check Symfony logs for Messenger processing
- Check browser console for Mercure connection status
- Verify Caddy/FrankenPHP is running with Mercure enabled
- Check that messenger:consume worker is running for async processing

## Future Enhancements

1. **Multiplayer Support**: Add opponent matching and publish updates for both players
2. **Reconnection Logic**: Handle network disconnections gracefully
3. **Move Animations**: Add smooth transitions when updates arrive
4. **Presence Indication**: Show when opponent is online/offline
5. **Typing Indicators**: Show when opponent is thinking/calculating move

## Security Considerations

1. **JWT Tokens**: Mercure uses JWT for authorization
   - Publisher token set via environment variable
   - Subscriber token can be restricted per topic
2. **Topic Namespacing**: Each game has unique UUID-based topic
3. **Message Validation**: All updates validated server-side before publishing
4. **Database Consistency**: Moves saved atomically with transactions

## Performance Notes

1. **Async Processing**: AI moves don't block HTTP response
2. **Efficient Updates**: Full board state sent (no delta computation needed)
3. **Timestamp Ordering**: Simple timestamp comparison prevents complex ordering logic
4. **Database Polling**: Messenger uses database transport (can be upgraded to Redis/AMQP for scale)
