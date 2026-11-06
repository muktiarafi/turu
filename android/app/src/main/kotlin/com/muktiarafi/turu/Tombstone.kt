package com.muktiarafi.turu

import java.util.Locale

/**
 * A native crash's tombstone (Android 12 and later hand it over as a protobuf: AOSP's
 * `system/core/debuggerd/proto/tombstone.proto`) as the text debuggerd writes: the signal, the
 * abort message, the crashing thread's backtrace, the other threads' tops and the end of the
 * process's log. Fields this doesn't know are skipped; the raw file is kept next to the report for
 * anything else (`pbtombstone`, or `protoc --decode` with that .proto).
 */
internal object Tombstone {
    /** The other threads' frames shown (there are dozens of threads: tokio, wgpu, audio). */
    private const val OTHER_THREAD_FRAMES = 6

    /** The log lines shown, from the end of each buffer. */
    private const val LOG_LINES = 300

    /** The text, or null when the bytes aren't a tombstone this can read. */
    fun format(data: ByteArray): String? = try {
        formatOrThrow(data)
    } catch (_: RuntimeException) {
        null
    }

    private class Frame {
        var relPc = 0L
        var function = ""
        var offset = 0L
        var file = ""
        var buildId = ""
        var fileOffset = 0L
    }

    private class Thread {
        var id = 0
        var name = ""
        val frames = mutableListOf<Frame>()
        val notes = mutableListOf<String>()
    }

    private class LogLine {
        var timestamp = ""
        var pid = 0L
        var tid = 0L
        var priority = 0L
        var tag = ""
        var message = ""
    }

    private class LogBuffer(val name: String, val lines: List<LogLine>)

    private fun formatOrThrow(data: ByteArray): String {
        var fingerprint = ""
        var timestamp = ""
        var pid = 0L
        var tid = 0L
        var uptime = -1L
        val cmdline = mutableListOf<String>()
        var signal: String? = null
        var abort: String? = null
        val causes = mutableListOf<String>()
        val threads = mutableListOf<Thread>()
        val logs = mutableListOf<LogBuffer>()

        val r = Reader(data, 0, data.size)
        while (r.more()) {
            val tag = r.tag()
            when (tag ushr 3) {
                2 -> fingerprint = r.string()
                4 -> timestamp = r.string()
                5 -> pid = r.varint()
                6 -> tid = r.varint()
                9 -> cmdline.add(r.string())
                10 -> signal = signal(r.message())
                14 -> abort = r.string()
                15 -> r.message().fields { field, t -> if (field == 1) causes.add(string()) else skip(t) }
                16 -> r.message().fields { field, t -> if (field == 2) threads.add(thread(message())) else skip(t) }
                18 -> {
                    var name = ""
                    val lines = mutableListOf<LogLine>()
                    r.message().fields { field, t ->
                        when (field) {
                            1 -> name = string()
                            2 -> lines.add(logLine(message()))
                            else -> skip(t)
                        }
                    }
                    logs.add(LogBuffer(name, lines))
                }
                20 -> uptime = r.varint()
                else -> r.skip(tag)
            }
        }

        val out = StringBuilder()
        out.append("Build fingerprint: '").append(fingerprint).append("'\n")
        out.append("Timestamp: ").append(timestamp).append('\n')
        if (uptime >= 0) {
            out.append("Process uptime: ").append(uptime).append("s\n")
        }
        out.append("Cmdline: ").append(cmdline.joinToString(" ")).append('\n')
        val crashed = threads.lastOrNull { it.id.toLong() == tid }
        out.append("pid: ").append(pid).append(", tid: ").append(tid).append(", name: ")
            .append(crashed?.name ?: "?").append('\n')
        signal?.let { out.append(it).append('\n') }
        if (!abort.isNullOrEmpty()) {
            out.append("Abort message: '").append(abort).append("'\n")
        }
        for (cause in causes) {
            out.append("Cause: ").append(cause).append('\n')
        }
        if (crashed != null) {
            out.append("\nbacktrace (thread \"").append(crashed.name).append("\"):\n")
            appendFrames(out, crashed, Int.MAX_VALUE)
        }
        for (thread in threads) {
            if (thread === crashed) {
                continue
            }
            out.append("\nthread ").append(thread.id).append(" \"").append(thread.name).append("\":\n")
            appendFrames(out, thread, OTHER_THREAD_FRAMES)
        }
        for (log in logs) {
            val lines = log.lines
            out.append("\nlog ").append(log.name).append(" (the last ")
                .append(minOf(lines.size, LOG_LINES)).append(" lines):\n")
            for (line in lines.takeLast(LOG_LINES)) {
                out.append(line.timestamp).append(' ').append(line.pid).append(' ').append(line.tid).append(' ')
                    .append(priority(line.priority)).append(' ').append(line.tag).append(": ")
                    .append(line.message).append('\n')
            }
        }
        return out.toString()
    }

    private fun appendFrames(out: StringBuilder, thread: Thread, max: Int) {
        for (note in thread.notes) {
            out.append("      NOTE: ").append(note).append('\n')
        }
        val shown = minOf(max, thread.frames.size)
        for (i in 0 until shown) {
            val frame = thread.frames[i]
            out.append(String.format(Locale.US, "      #%02d pc %016x  %s", i, frame.relPc, frame.file))
            // A library inside the APK: where its ELF starts in the APK.
            if (frame.fileOffset != 0L) {
                out.append(String.format(Locale.US, " (offset 0x%x)", frame.fileOffset))
            }
            if (frame.function.isNotEmpty()) {
                out.append(" (").append(frame.function).append('+').append(frame.offset).append(')')
            }
            if (frame.buildId.isNotEmpty()) {
                out.append(" (BuildId: ").append(frame.buildId).append(')')
            }
            out.append('\n')
        }
        if (shown < thread.frames.size) {
            out.append("      ... ").append(thread.frames.size - shown).append(" more\n")
        }
    }

    private fun signal(r: Reader): String {
        var number = 0L
        var name = ""
        var code = 0L
        var codeName = ""
        var hasSender = false
        var senderPid = 0L
        var hasFault = false
        var fault = 0L
        r.fields { field, tag ->
            when (field) {
                1 -> number = varint()
                2 -> name = string()
                3 -> code = varint()
                4 -> codeName = string()
                5 -> hasSender = varint() != 0L
                7 -> senderPid = varint()
                8 -> hasFault = varint() != 0L
                9 -> fault = varint()
                else -> skip(tag)
            }
        }
        val out = StringBuilder()
        out.append("signal ").append(number).append(" (").append(name).append("), code ").append(code.toInt())
            .append(" (").append(codeName)
        if (hasSender) {
            out.append(" from pid ").append(senderPid)
        }
        out.append("), fault addr ").append(if (hasFault) String.format(Locale.US, "0x%x", fault) else "--------")
        return out.toString()
    }

    private fun thread(r: Reader): Thread {
        val thread = Thread()
        r.fields { field, tag ->
            when (field) {
                1 -> thread.id = varint().toInt()
                2 -> thread.name = string()
                4 -> thread.frames.add(frame(message()))
                7 -> thread.notes.add(string())
                else -> skip(tag)
            }
        }
        return thread
    }

    private fun frame(r: Reader): Frame {
        val frame = Frame()
        r.fields { field, tag ->
            when (field) {
                1 -> frame.relPc = varint()
                4 -> frame.function = string()
                5 -> frame.offset = varint()
                6 -> frame.file = string()
                7 -> frame.fileOffset = varint()
                8 -> frame.buildId = string()
                else -> skip(tag)
            }
        }
        return frame
    }

    private fun logLine(r: Reader): LogLine {
        val line = LogLine()
        r.fields { field, tag ->
            when (field) {
                1 -> line.timestamp = string()
                2 -> line.pid = varint()
                3 -> line.tid = varint()
                4 -> line.priority = varint()
                5 -> line.tag = string()
                6 -> line.message = string()
                else -> skip(tag)
            }
        }
        return line
    }

    private fun priority(priority: Long): Char =
        if (priority in 2..7) "VDIWEF"[(priority - 2).toInt()] else '?'

    /** Protobuf's wire format: tags, varints, length-delimited fields; the rest is skipped. */
    private class Reader(private val data: ByteArray, private var pos: Int, private val end: Int) {
        fun more(): Boolean = pos < end

        fun tag(): Int = varint().toInt()

        /** Reads each field to the end: `read(fieldNumber, tag)`, which reads or skips its value. */
        inline fun fields(read: Reader.(field: Int, tag: Int) -> Unit) {
            while (more()) {
                val tag = tag()
                read(tag ushr 3, tag)
            }
        }

        fun varint(): Long {
            var value = 0L
            var shift = 0
            while (shift < 64) {
                check(pos < end) { "truncated varint" }
                val b = data[pos++].toInt()
                value = value or ((b and 0x7f).toLong() shl shift)
                if (b and 0x80 == 0) {
                    return value
                }
                shift += 7
            }
            throw IllegalStateException("varint too long")
        }

        private fun length(): Int {
            val length = varint()
            check(length >= 0 && length <= end - pos) { "bad length" }
            return length.toInt()
        }

        fun message(): Reader {
            val length = length()
            val sub = Reader(data, pos, pos + length)
            pos += length
            return sub
        }

        fun string(): String {
            val length = length()
            val s = String(data, pos, length, Charsets.UTF_8)
            pos += length
            return s
        }

        fun skip(tag: Int) {
            when (tag and 7) {
                0 -> varint()
                1 -> advance(8)
                2 -> advance(length())
                5 -> advance(4)
                else -> throw IllegalStateException("wire type ${tag and 7}")
            }
        }

        private fun advance(n: Int) {
            check(n <= end - pos) { "truncated" }
            pos += n
        }
    }
}
