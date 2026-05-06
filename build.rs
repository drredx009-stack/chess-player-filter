fn main() {
    let mut res = winres::WindowsResource::new();
    res.set("ProductName", "Chess Player Filter");
    res.set("FileDescription", "Chess.com account analyzer");
    res.set_icon("iocn.ico");
    res.compile().unwrap();
}