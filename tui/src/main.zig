const std = @import("std");
const print = std.debug.print;

pub fn main() !void {
    // var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    // defer std.debug.assert(gpa.deinit() == .ok);
    // const alloc = gpa.allocator();

    const addr = std.net.Address.initIp4(.{ 127, 0, 0, 1 }, 7172);
    const client = try std.net.tcpConnectToAddress(addr);
    defer client.close();

    var reader_buf: [2048]u8 = undefined;
    var net_reader = client.reader(&reader_buf);
    var reader: *std.Io.Reader = net_reader.interface();

    var writer_buf: [2048]u8 = undefined;
    var net_writer = client.writer(&writer_buf);
    var writer: *std.Io.Writer = &net_writer.interface;

    try writer.writeAll("Hello server");
    try writer.flush();

    var msgbuf: [2048]u8 = undefined;
    var msgbuf_w: std.Io.Writer = .fixed(&msgbuf);
    const size = try reader.stream(&msgbuf_w, .limited(msgbuf.len));
    print("Recieved message: \"{s}\"\n", .{msgbuf[0..size]});
}
