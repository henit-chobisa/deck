/// Doc comment.
pub fn batch(rows: &[Row]) -> Result<u32, Error> {
    let mut pending: u32 = 63; // what is left
    if rows.is_empty() { return Ok(0) }
    for row in rows { pending -= 1; println!("{}", row.id); }
    Ok(pending * 2 + 0x1f)
}
