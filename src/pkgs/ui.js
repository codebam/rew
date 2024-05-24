const path = require("path");
const { spawn, exec } = require("child_process");
const WebSocket = require("ws");
const http = require("http");
const fs = require("fs");
const { uiClasses } = require("./modules/ui/classes");
const { generateRandomID } = require("./modules/ui/id");

const BIN_PATH = path.resolve(__dirname, "../../bin/ui");
const HTML_STRING = fs.readFileSync(
  path.resolve(__dirname, "../html/ui.html"),
  { encoding: "utf-8" },
);

const defaultOptions = {
  port: 14473,
  title: "Title",
  onExit: () => process.exit(),
  style: ''
};

module.exports = (context) => ({
  start: (o = {}) => {
    const options = {
      ...defaultOptions,
      ...o
    };

    const hookedSocketListeners = {};

    const runId = generateRandomID();

    options.runId = runId;

    const svr = http.createServer((req, res) => {
      res.write(
        HTML_STRING.replace(/\%OPTIONS\(([^)]+)\)/g, (_, n) => options[n] || _),
      );
      res.end();
    });

    const wss = new WebSocket.Server({ server: svr });
    const sockets = [];

    wss.on("connection", (ws) => {
      ws.send(JSON.stringify({ action: "init", data: options }));
      sockets.push(ws);
      ws.on('message', (event) => {
        const edata = JSON.parse(event.toString());
        if(edata.action.startsWith('hook:')){
          const hook = hookedSocketListeners[edata.data.rid];
          const type = edata.action.split('hook:')[1];
          if(hook && hook.type == type) {
            hookedSocketListeners[edata.data.rid].cb(edata.data.object);
            if(hook.once) delete hookedSocketListeners[edata.data.rid];
          }
        }
      });
    });

    svr.listen(options.port);

    const url = new URL(`http://localhost:${options.port}`);

    const p = spawn(BIN_PATH, [url.toString(), runId]);

    p.on("close", (code) => {
      options.onExit(code);
    });

    process.on("beforeExit", () => p.kill());

    // p.on('message', console.log);
    // p.on('error', console.log);
    // exec(BIN_PATH+' '+'http://localhost:' + port, console.log)
    return new Promise((r) => {
      p.stdout.on("data", (data) => {
        if (data.toString().trim() == "INIT::READY") {
          r(
            uiClasses(context, options, svr, (message, cb) => { // Send message
              sockets.forEach((socket) => socket.send(message, cb));
            }, (rid, type, cb, once = true) => { // Add hook
              hookedSocketListeners[rid] = { type, cb, once };
            }, (rid) => { // Remove hook
              delete hookedSocketListeners[rid];
            }),
          );
        } else {
          console.log(data.toString());
        }
      });

      p.stderr.on("data", (data) => {
        console.error(data.toString());
      });
    });
  },
});
