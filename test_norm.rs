use miniquad::*;
fn main() {
    let fmt = VertexFormat::Byte4;
    let desc = fmt.size();
    println!("{:?}", desc);
}
