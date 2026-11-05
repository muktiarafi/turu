package com.muktiarafi.turu;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * A native crash's tombstone (Android 12 and later hand it over as a protobuf: AOSP's
 * {@code system/core/debuggerd/proto/tombstone.proto}) as the text debuggerd writes: the signal,
 * the abort message, the crashing thread's backtrace, the other threads' tops and the end of the
 * process's log. Fields this doesn't know are skipped; the raw file is kept next to the report for
 * anything else ({@code pbtombstone}, or {@code protoc --decode} with that .proto).
 */
final class Tombstone {
    /** The other threads' frames shown (there are dozens of threads: tokio, wgpu, audio). */
    private static final int OTHER_THREAD_FRAMES = 6;
    /** The log lines shown, from the end of each buffer. */
    private static final int LOG_LINES = 300;

    private Tombstone() {}

    /** The text, or null when the bytes aren't a tombstone this can read. */
    static String format(byte[] data) {
        try {
            return formatOrThrow(data);
        } catch (RuntimeException e) {
            return null;
        }
    }

    private static final class Frame {
        long relPc;
        String function = "";
        long offset;
        String file = "";
        String buildId = "";
        long fileOffset;
    }

    private static final class Thread {
        int id;
        String name = "";
        final List<Frame> frames = new ArrayList<>();
        final List<String> notes = new ArrayList<>();
    }

    private static final class LogLine {
        String timestamp = "";
        long pid;
        long tid;
        long priority;
        String tag = "";
        String message = "";
    }

    private static String formatOrThrow(byte[] data) {
        String fingerprint = "";
        String timestamp = "";
        long pid = 0;
        long tid = 0;
        long uptime = -1;
        List<String> cmdline = new ArrayList<>();
        String signal = null;
        String abort = null;
        List<String> causes = new ArrayList<>();
        List<Thread> threads = new ArrayList<>();
        List<String> logNames = new ArrayList<>();
        List<List<LogLine>> logs = new ArrayList<>();

        Reader r = new Reader(data, 0, data.length);
        while (r.more()) {
            int tag = r.tag();
            switch (tag >>> 3) {
                case 2: fingerprint = r.string(); break;
                case 4: timestamp = r.string(); break;
                case 5: pid = r.varint(); break;
                case 6: tid = r.varint(); break;
                case 9: cmdline.add(r.string()); break;
                case 10: signal = signal(r.message()); break;
                case 14: abort = r.string(); break;
                case 15: {
                    Reader cause = r.message();
                    while (cause.more()) {
                        int t = cause.tag();
                        if (t >>> 3 == 1) {
                            causes.add(cause.string());
                        } else {
                            cause.skip(t);
                        }
                    }
                    break;
                }
                case 16: {
                    Reader entry = r.message();
                    while (entry.more()) {
                        int t = entry.tag();
                        if (t >>> 3 == 2) {
                            threads.add(thread(entry.message()));
                        } else {
                            entry.skip(t);
                        }
                    }
                    break;
                }
                case 18: {
                    Reader buffer = r.message();
                    String name = "";
                    List<LogLine> lines = new ArrayList<>();
                    while (buffer.more()) {
                        int t = buffer.tag();
                        if (t >>> 3 == 1) {
                            name = buffer.string();
                        } else if (t >>> 3 == 2) {
                            lines.add(logLine(buffer.message()));
                        } else {
                            buffer.skip(t);
                        }
                    }
                    logNames.add(name);
                    logs.add(lines);
                    break;
                }
                case 20: uptime = r.varint(); break;
                default: r.skip(tag);
            }
        }

        StringBuilder out = new StringBuilder();
        out.append("Build fingerprint: '").append(fingerprint).append("'\n");
        out.append("Timestamp: ").append(timestamp).append('\n');
        if (uptime >= 0) {
            out.append("Process uptime: ").append(uptime).append("s\n");
        }
        out.append("Cmdline: ").append(String.join(" ", cmdline)).append('\n');
        Thread crashed = null;
        for (Thread thread : threads) {
            if (thread.id == tid) {
                crashed = thread;
            }
        }
        out.append("pid: ").append(pid).append(", tid: ").append(tid).append(", name: ")
                .append(crashed != null ? crashed.name : "?").append('\n');
        if (signal != null) {
            out.append(signal).append('\n');
        }
        if (abort != null && !abort.isEmpty()) {
            out.append("Abort message: '").append(abort).append("'\n");
        }
        for (String cause : causes) {
            out.append("Cause: ").append(cause).append('\n');
        }
        if (crashed != null) {
            out.append("\nbacktrace (thread \"").append(crashed.name).append("\"):\n");
            appendFrames(out, crashed, Integer.MAX_VALUE);
        }
        for (Thread thread : threads) {
            if (thread == crashed) {
                continue;
            }
            out.append("\nthread ").append(thread.id).append(" \"").append(thread.name).append("\":\n");
            appendFrames(out, thread, OTHER_THREAD_FRAMES);
        }
        for (int i = 0; i < logs.size(); i++) {
            List<LogLine> lines = logs.get(i);
            out.append("\nlog ").append(logNames.get(i)).append(" (the last ")
                    .append(Math.min(lines.size(), LOG_LINES)).append(" lines):\n");
            for (LogLine line : lines.subList(Math.max(0, lines.size() - LOG_LINES), lines.size())) {
                out.append(line.timestamp).append(' ').append(line.pid).append(' ').append(line.tid).append(' ')
                        .append(priority(line.priority)).append(' ').append(line.tag).append(": ")
                        .append(line.message).append('\n');
            }
        }
        return out.toString();
    }

    private static void appendFrames(StringBuilder out, Thread thread, int max) {
        for (String note : thread.notes) {
            out.append("      NOTE: ").append(note).append('\n');
        }
        int shown = Math.min(max, thread.frames.size());
        for (int i = 0; i < shown; i++) {
            Frame frame = thread.frames.get(i);
            out.append(String.format(Locale.US, "      #%02d pc %016x  %s", i, frame.relPc, frame.file));
            // A library inside the APK: where its ELF starts in the APK.
            if (frame.fileOffset != 0) {
                out.append(String.format(Locale.US, " (offset 0x%x)", frame.fileOffset));
            }
            if (!frame.function.isEmpty()) {
                out.append(" (").append(frame.function).append('+').append(frame.offset).append(')');
            }
            if (!frame.buildId.isEmpty()) {
                out.append(" (BuildId: ").append(frame.buildId).append(')');
            }
            out.append('\n');
        }
        if (shown < thread.frames.size()) {
            out.append("      ... ").append(thread.frames.size() - shown).append(" more\n");
        }
    }

    private static String signal(Reader r) {
        long number = 0;
        String name = "";
        long code = 0;
        String codeName = "";
        boolean hasSender = false;
        long senderPid = 0;
        boolean hasFault = false;
        long fault = 0;
        while (r.more()) {
            int tag = r.tag();
            switch (tag >>> 3) {
                case 1: number = r.varint(); break;
                case 2: name = r.string(); break;
                case 3: code = r.varint(); break;
                case 4: codeName = r.string(); break;
                case 5: hasSender = r.varint() != 0; break;
                case 7: senderPid = r.varint(); break;
                case 8: hasFault = r.varint() != 0; break;
                case 9: fault = r.varint(); break;
                default: r.skip(tag);
            }
        }
        StringBuilder out = new StringBuilder();
        out.append("signal ").append(number).append(" (").append(name).append("), code ").append((int) code)
                .append(" (").append(codeName);
        if (hasSender) {
            out.append(" from pid ").append(senderPid);
        }
        out.append("), fault addr ").append(hasFault ? String.format(Locale.US, "0x%x", fault) : "--------");
        return out.toString();
    }

    private static Thread thread(Reader r) {
        Thread thread = new Thread();
        while (r.more()) {
            int tag = r.tag();
            switch (tag >>> 3) {
                case 1: thread.id = (int) r.varint(); break;
                case 2: thread.name = r.string(); break;
                case 4: thread.frames.add(frame(r.message())); break;
                case 7: thread.notes.add(r.string()); break;
                default: r.skip(tag);
            }
        }
        return thread;
    }

    private static Frame frame(Reader r) {
        Frame frame = new Frame();
        while (r.more()) {
            int tag = r.tag();
            switch (tag >>> 3) {
                case 1: frame.relPc = r.varint(); break;
                case 4: frame.function = r.string(); break;
                case 5: frame.offset = r.varint(); break;
                case 6: frame.file = r.string(); break;
                case 7: frame.fileOffset = r.varint(); break;
                case 8: frame.buildId = r.string(); break;
                default: r.skip(tag);
            }
        }
        return frame;
    }

    private static LogLine logLine(Reader r) {
        LogLine line = new LogLine();
        while (r.more()) {
            int tag = r.tag();
            switch (tag >>> 3) {
                case 1: line.timestamp = r.string(); break;
                case 2: line.pid = r.varint(); break;
                case 3: line.tid = r.varint(); break;
                case 4: line.priority = r.varint(); break;
                case 5: line.tag = r.string(); break;
                case 6: line.message = r.string(); break;
                default: r.skip(tag);
            }
        }
        return line;
    }

    private static char priority(long priority) {
        return priority >= 2 && priority <= 7 ? "VDIWEF".charAt((int) priority - 2) : '?';
    }

    /** Protobuf's wire format: tags, varints, length-delimited fields; the rest is skipped. */
    private static final class Reader {
        private final byte[] data;
        private int pos;
        private final int end;

        Reader(byte[] data, int pos, int end) {
            this.data = data;
            this.pos = pos;
            this.end = end;
        }

        boolean more() {
            return pos < end;
        }

        int tag() {
            return (int) varint();
        }

        long varint() {
            long value = 0;
            for (int shift = 0; shift < 64; shift += 7) {
                if (pos >= end) {
                    throw new IllegalStateException("truncated varint");
                }
                byte b = data[pos++];
                value |= (long) (b & 0x7f) << shift;
                if ((b & 0x80) == 0) {
                    return value;
                }
            }
            throw new IllegalStateException("varint too long");
        }

        private int length() {
            long length = varint();
            if (length < 0 || length > end - pos) {
                throw new IllegalStateException("bad length");
            }
            return (int) length;
        }

        Reader message() {
            int length = length();
            Reader sub = new Reader(data, pos, pos + length);
            pos += length;
            return sub;
        }

        String string() {
            int length = length();
            String s = new String(data, pos, length, StandardCharsets.UTF_8);
            pos += length;
            return s;
        }

        void skip(int tag) {
            switch (tag & 7) {
                case 0: varint(); break;
                case 1: advance(8); break;
                case 2: advance(length()); break;
                case 5: advance(4); break;
                default: throw new IllegalStateException("wire type " + (tag & 7));
            }
        }

        private void advance(int n) {
            if (n > end - pos) {
                throw new IllegalStateException("truncated");
            }
            pos += n;
        }
    }
}
