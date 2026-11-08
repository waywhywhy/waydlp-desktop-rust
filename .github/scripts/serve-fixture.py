"""Generate a one-second tone and serve it for real download smoke tests."""
import functools
import http.server
import math
from pathlib import Path
import struct
import tempfile
import wave

with tempfile.TemporaryDirectory(prefix="waydlp-smoke-") as directory:
    with wave.open(str(Path(directory) / "tone.wav"), "wb") as audio:
        audio.setparams((1, 2, 44100, 44100, "NONE", "not compressed"))
        audio.writeframes(b"".join(
            struct.pack("<h", int(12000 * math.sin(2 * math.pi * 440 * i / 44100)))
            for i in range(44100)
        ))
    handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=directory)
    print("Serving generated fixture on port 8765", flush=True)
    http.server.ThreadingHTTPServer(("0.0.0.0", 8765), handler).serve_forever()
