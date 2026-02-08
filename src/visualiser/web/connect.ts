import config from "./config.json" with { type: "json" }

type Conn = { readable: ReadableStream, writable: WritableStream };

async function connectWebTransport(): Promise<Conn> {
    let certHash = (await import(config.certHashUrl, { with: { type: "json" } })).default as number[];

    let wt = new WebTransport(config.robotUrl, {
        serverCertificateHashes: [{
            algorithm: "sha-256",
            value: new Uint8Array(certHash),
        }],
    });
    await wt.ready;
    return await wt.createBidirectionalStream();
}

export async function connect() { return await connectWebTransport(); }