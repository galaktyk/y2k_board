const fs = require('fs');

let files = ['src/toolbar.rs', 'src/stats.rs', 'src/text.rs', 'src/input/handles.rs'];

for (let file of files) {
  let r = fs.readFileSync(file, 'utf8');
  let count = 0;
  
  let re = /InstanceData\s*\{\s*pos:\s*(.+?),\s*size:\s*(.+?),\s*rotation:\s*(.+?),\s*color:\s*(.+?),\s*shape_type:\s*(.+?),\s*alpha:\s*(.+?),?\s*\}/gs;

  r = r.replace(re, (match, p, s, rot, col, st, a) => {
    count++;
    return `InstanceData::new(${p}, ${s}, ${rot}, ${col}, ${st}, ${a})`;
  });

  let re_txt = /TextInstanceData\s*\{\s*pos:\s*(.+?),\s*size:\s*(.+?),\s*origin:\s*(.+?),\s*rotation:\s*(.+?),\s*uv_min:\s*(.+?),\s*uv_max:\s*(.+?),\s*color:\s*(.+?),?\s*\}/gs;
  r = r.replace(re_txt, (match, p, s, o, rot, u_min, u_max, col) => {
    return `TextInstanceData::new(${p}, ${s}, ${o}, ${rot}, ${u_min}, ${u_max}, ${col})`;
  });

  fs.writeFileSync(file, r);
  console.log(file, count);
}
