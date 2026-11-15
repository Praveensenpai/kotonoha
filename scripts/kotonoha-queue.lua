-- kotonoha-queue.lua
-- Press Shift+Q (capital Q) in mpv to queue the current video timestamp.
-- Destination: ~/.local/share/kotonoha/queue.jsonl (or $XDG_DATA_HOME/kotonoha/queue.jsonl)

local utils = require("mp.utils")

local function get_queue_file_path()
    local xdg_data = os.getenv("XDG_DATA_HOME")
    local data_dir
    if xdg_data and xdg_data ~= "" then
        data_dir = utils.join_path(xdg_data, "kotonoha")
    else
        local home = os.getenv("HOME") or ""
        data_dir = utils.join_path(home, ".local/share/kotonoha")
    end
    -- Create directory if needed
    os.execute(string.format('mkdir -p "%s"', data_dir))
    return utils.join_path(data_dir, "queue.jsonl")
end

mp.add_key_binding("Q", "kotonoha-queue", function()
    local path = mp.get_property("path")
    local pos = mp.get_property_number("time-pos")
    if not path or not pos then
        mp.osd_message("kotonoha: no media active or timestamp unavailable", 2)
        return
    end

    local abs_path = path
    -- If path is relative and not a URL, resolve against working-directory
    if not path:match("^/") and not path:match("^%a+://") then
        local cwd = mp.get_property("working-directory") or ""
        abs_path = utils.join_path(cwd, path)
    end

    local ts_ms = math.floor(pos * 1000)
    local queue_path = get_queue_file_path()

    -- JSON escape the path
    local escaped_path = abs_path:gsub("\\", "\\\\"):gsub('"', '\\"'):gsub("\n", "\\n")
    local line = string.format('{"video_path":"%s","timestamp_ms":%d}\n', escaped_path, ts_ms)

    local f, err = io.open(queue_path, "a")
    if f then
        f:write(line)
        f:close()
        mp.osd_message(string.format("⏱ Queued at %s", mp.format_time(pos)), 2)
    else
        mp.osd_message("kotonoha: failed to write queue file", 3)
        mp.msg.error(string.format("Failed to open queue file %s: %s", queue_path, tostring(err)))
    end
end)
