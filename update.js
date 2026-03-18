const fs = require('fs');
let code = fs.readFileSync('src/input/handlers/mouse.rs', 'utf8');

// replace the hit tests!
code = code.replace(
    /let hit_target_start = board\.hit_test\(start_pos\)\.filter\(\|\&h_id\| h_id \!\= id\);/g,
    'let hit_target_start = board.hit_test_all(start_pos).into_iter().find(|&h_id| h_id != id && board.element(h_id).map(|e| e.shape != ShapeType::Line).unwrap_or(false));'
);
code = code.replace(
    /let hit_target_end = board\.hit_test\(end_pos\)\.filter\(\|\&h_id\| h_id \!\= id\);/g,
    'let hit_target_end = board.hit_test_all(end_pos).into_iter().find(|&h_id| h_id != id && board.element(h_id).map(|e| e.shape != ShapeType::Line).unwrap_or(false));'
);

code = code.replace(
    /let hit_target_start = board\.hit_test\(start_pos\)\.filter\(\|\&h_id\| h_id \!\= new_id\);/g,
    'let hit_target_start = board.hit_test_all(start_pos).into_iter().find(|&h_id| h_id != new_id && board.element(h_id).map(|e| e.shape != ShapeType::Line).unwrap_or(false));'
);
code = code.replace(
    /let hit_target_end = board\.hit_test\(end_pos\)\.filter\(\|\&h_id\| h_id \!\= new_id\);/g,
    'let hit_target_end = board.hit_test_all(end_pos).into_iter().find(|&h_id| h_id != new_id && board.element(h_id).map(|e| e.shape != ShapeType::Line).unwrap_or(false));'
);

let blockStartMatch = code.indexOf('if matches!(state.drag_mode, DragMode::ResizingHandle(_) | DragMode::MoveSelected) {');
let lines = code.split('\n');
let blockStartLine = lines.findIndex(l => l.includes('if matches!(state.drag_mode, DragMode::ResizingHandle(_) | DragMode::MoveSelected) {'));

// find closing brace of that if matches! block.
let openBraces = 0;
let blockEndLine = -1;
let started = false;
for (let i = blockStartLine; i < lines.length; i++) {
    if (!started && lines[i].includes('{')) {
        started = true;
    }
    openBraces += (lines[i].match(/\{/g) || []).length;
    openBraces -= (lines[i].match(/\}/g) || []).length;
    if (started && openBraces === 0) {
        blockEndLine = i;
        break;
    }
}

let extractedBlock = lines.slice(blockStartLine, blockEndLine + 1).join('\n');
code = code.replace(extractedBlock, '');

code = code.replace('state.drag_mode = DragMode::None;', extractedBlock + '\n    state.drag_mode = DragMode::None;');

fs.writeFileSync('src/input/handlers/mouse.rs', code);
