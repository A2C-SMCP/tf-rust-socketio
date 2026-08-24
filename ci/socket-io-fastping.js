// Socket.IO test server with a fast heartbeat (pingInterval 300ms /
// pingTimeout 700ms) — mirrors rust-sdk#208 field parameters. Used to prove
// that long-running event callbacks no longer stall Engine.IO pings (issue #12).
const server = require('http').createServer();
const io = require('socket.io')(server, { pingInterval: 300, pingTimeout: 700 });

console.log('Started');
var callback = client => {
    console.log('Connected!');
    client.on('test', data => {
        // Send a message back to the server to confirm the message was received
        client.emit('test-received', data);
        console.log(['test', data]);
    });
    client.on('message', data => {
        client.emit('message-received', data);
        console.log(['message', data]);
    });
    client.on('test', function (arg, ack) {
        console.log('Ack received')
        if (ack) {
            ack('woot');
        }
    });

    client.on('binary', data => {
        var bufView = new Uint8Array(data);
        console.log(['binary', 'Yehaa binary payload!']);
        for (elem in bufView) {
            console.log(['binary', elem]);
        }
        client.emit('binary-received', data);
        console.log(['binary', data]);
    });

    client.on('binary', function (arg, ack) {
        console.log(['binary', 'Ack received, answer with binary'])
        if (ack) {
            ack(Buffer.from([1, 2, 3]));
        }
    });

    // This event allows the test framework to arbitrarily close the underlying connection
    client.on('close_transport', data => {
        console.log(['close_transport', 'Request to close transport received'])
        // Close underlying websocket connection
        client.client.conn.close();
    })

    client.emit('Hello from the message event!');
    client.emit('test', 'Hello from the test event!');
    client.emit(Buffer.from([4, 5, 6]));
    client.emit('test', Buffer.from([1, 2, 3]));
    client.emit('This is the first argument', 'This is the second argument', {
        argCount: 3
    });
    client.emit('on_abc_event', '', {
        abc: 0,
        some_other: 'value',
    });
};
io.on('connection', callback);
io.of('/admin').on('connection', callback);
// the socket.io fast heartbeat client runs on port 4207
server.listen(4207);
