import config from "./config.json" with { type: "json" }
import { Connection } from "./visualiser.js";

async function connectWebTransport(): Promise<Connection> {
    let certHash = (await import(config.certHashUrl, { with: { type: "json" } })).default as number[];

    let wt = new WebTransport(config.robotUrl, {
        serverCertificateHashes: [{
            algorithm: "sha-256",
            value: new Uint8Array(certHash),
        }],
    });
    await wt.ready;
    let bidistream = await wt.createBidirectionalStream();
    return new Connection(
        bidistream.readable,
        bidistream.writable,
        wt.close.bind(wt),
    )
}

export async function connect() { return await connectWebTransport(); }