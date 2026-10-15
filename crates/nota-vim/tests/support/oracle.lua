-- Oráculo: aplica teclas en nvim y escribe cursor, modo y texto en un archivo.
-- Uso: nvim --clean --headless -n -l oracle.lua <texto> <fila> <columna> <teclas> <salida>
local text, row, col, keys, out = arg[1], tonumber(arg[2]), tonumber(arg[3]), arg[4], arg[5]

-- nota todavía no implementa autoindent.
vim.o.autoindent = false

-- Cargar el texto sin dejarlo en el historial de undo.
vim.o.undolevels = -1
vim.api.nvim_buf_set_lines(0, 0, -1, true, vim.split(text, "\n", { plain = true }))
vim.o.undolevels = 1000
vim.api.nvim_win_set_cursor(0, { row, col })

function _G.NotaOracleDump()
  local pos = vim.api.nvim_win_get_cursor(0)
  local file = assert(io.open(out, "w"))
  file:write(string.format("%d %d %s\n", pos[1], pos[2], vim.api.nvim_get_mode().mode))
  file:write(table.concat(vim.api.nvim_buf_get_lines(0, 0, -1, true), "\n"))
  file:close()
end

-- <Cmd> lee el estado sin salir del modo actual (Insert incluido).
local sequence = keys .. "<Cmd>lua NotaOracleDump()<CR>"
vim.api.nvim_feedkeys(vim.api.nvim_replace_termcodes(sequence, true, false, true), "xtn", false)
vim.cmd("qa!")
