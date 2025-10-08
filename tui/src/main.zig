const std = @import("std");
const print = std.debug.print;

const rw_buf_size = 512;
const max_msg_size = 1024;

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer std.debug.assert(gpa.deinit() == .ok);
    const alloc = gpa.allocator();

    var args = try std.process.argsWithAllocator(alloc);

    const addr = std.net.Address.initIp4(.{ 127, 0, 0, 1 }, 7172);
    const client = try std.net.tcpConnectToAddress(addr);
    defer client.close();

    var reader_buf: [rw_buf_size]u8 = undefined;
    var net_reader = client.reader(&reader_buf);
    const reader: *std.Io.Reader = net_reader.interface();

    var writer_buf: [rw_buf_size]u8 = undefined;
    var net_writer = client.writer(&writer_buf);
    const writer: *std.Io.Writer = &net_writer.interface;

    _ = args.next();
    const cmd = args.next();
    if (cmd == null) {
        printhelp();
    } else if (std.mem.eql(u8, cmd.?, "posts")) {
        try posts(reader, writer);
    } else if (std.mem.eql(u8, cmd.?, "auth")) {
        try auth(reader, writer);
    } else {
        printhelp();
    }
}

pub fn printhelp() void {
    print("Invalid command\n", .{});
}

pub fn posts(reader: *std.Io.Reader, writer: *std.Io.Writer) !void {
    try writer.writeAll("posts\x00end");
    try writer.flush();

    var msgbuf: [max_msg_size]u8 = undefined;
    var msgbuf_w: std.Io.Writer = .fixed(&msgbuf);
    var n: usize = 0;
    while (n == 0) {
        n = try reader.stream(&msgbuf_w, .limited(msgbuf.len));
    }
    print("Recieved message: \"{s}\"\n", .{msgbuf[0..n]});
}

pub fn auth(reader: *std.Io.Reader, writer: *std.Io.Writer) !void {
    try writer.writeAll("auth\x00zacoons\x00a\x00end");
    try writer.flush();

    var msgbuf: [max_msg_size]u8 = undefined;
    var msgbuf_w: std.Io.Writer = .fixed(&msgbuf);
    var n: usize = 0;
    while (n == 0) {
        n = try reader.stream(&msgbuf_w, .limited(msgbuf.len));
    }
    print("Recieved message: \"{s}\"\n", .{msgbuf[0..n]});
}
