use tokio::io::BufReader;
use tokio_util::compat::{Compat, FuturesAsyncReadCompatExt, FuturesAsyncWriteCompatExt};
use wasm_bindgen::prelude::*;
use wasm_streams::{readable::IntoAsyncRead, writable::IntoAsyncWrite};
use web_sys::{ReadableStream, WritableStream, js_sys::Function};

type Reader = BufReader<Compat<IntoAsyncRead<'static>>>;
type Writer = Compat<IntoAsyncWrite<'static>>;

#[wasm_bindgen]
pub struct Connection {
    read: Reader,
    write: Writer,
    close: Function,
}

impl Drop for Connection {
    fn drop(&mut self) {
        match self.close.call0(&JsValue::null()) {
            Ok(_) => {}
            Err(e) => web_sys::console::error_1(&e),
        }
    }
}

impl interpreter::puzzle_states::Connection for Connection {
    type Reader = Reader;
    type Writer = Writer;

    fn reader(&mut self) -> &mut Self::Reader {
        &mut self.read
    }

    fn writer(&mut self) -> &mut Self::Writer {
        &mut self.write
    }
}

#[wasm_bindgen]
impl Connection {
    #[wasm_bindgen(constructor)]
    pub fn new(
        readable: ReadableStream,
        writable: WritableStream,
        #[wasm_bindgen(unchecked_param_type = "() => void")] close: Function,
    ) -> Self {
        Self {
            read: BufReader::new(
                wasm_streams::ReadableStream::from_raw(readable)
                    .into_async_read()
                    .compat(),
            ),
            write: wasm_streams::WritableStream::from_raw(writable)
                .into_async_write()
                .compat_write(),
            close,
        }
    }
}
