use std::io;
use std::path::PathBuf;


pub async fn read<P: Into<PathBuf>>(path: P) -> io::Result<Vec<u8>> {
    async fn read(path: PathBuf) -> io::Result<Vec<u8>> {
        crate::spawn(move || std::fs::read(path)).await
    }
    
    read(path.into()).await
}

pub async fn write<P: Into<PathBuf>, B: Into<Vec<u8>>>(path: P, bytes: B) -> io::Result<()> {
    async fn write(path: PathBuf, bytes: Vec<u8>) -> io::Result<()> {
        crate::spawn(move || std::fs::write(path, bytes)).await
    }

    write(path.into(), bytes.into()).await
}
