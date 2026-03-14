const fs = require('fs');

let file = 'src/text.rs';
let r = fs.readFileSync(file, 'utf8');

// For text.rs manual replacement
r = r.replace(/TextInstanceData \{\s*pos:\s*(.+?),\s*size:\s*(.+?),\s*origin,?\s*rotation:\s*(.+?),\s*uv_min:\s*(.+?),\s*uv_max:\s*(.+?),\s*color:\s*(.+?),?\s*\};/g, 
  "TextInstanceData::new($1, $2, origin, $3, $4, $5, $6);");

r = r.replace(/TextInstanceData \{\s*pos:\s*(.+?),\s*size:\s*(.+?),\s*origin,?\s*rotation:\s*(.+?),\s*uv_min,?\s*uv_max,?\s*color:\s*(.+?),?\s*\}\)/g, 
  "TextInstanceData::new($1, $2, origin, $3, uv_min, uv_max, $4))");

fs.writeFileSync(file, r);
