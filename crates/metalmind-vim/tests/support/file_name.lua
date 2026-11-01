-- Escribe, por cada argumento de `:e`, el nombre del archivo que abre nvim.
-- Uso: nvim --clean --headless -n -l file_name.lua <salida> <argumento>...
local out = assert(io.open(arg[1], "w"))
for i = 2, #arg do
  vim.cmd("silent! edit " .. arg[i])
  out:write(vim.fn.bufname() .. "\n")
  vim.cmd("silent! enew! | silent! %bwipeout!")
end
out:close()
