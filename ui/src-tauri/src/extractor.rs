/// Classifies a `chdman info` invocation's stdout as `"cd"` (created via
/// `createcd`, carries CD track metadata), `"dvd"` (created via `createdvd`,
/// carries a bare DVD tag), or `"unknown"` (neither pattern found -- not a
/// CHD chdman recognizes, or a format this app doesn't handle). This must be
/// known upfront before calling `extractcd`/`extractdvd`: those commands
/// don't validate that the input CHD matches, and silently write garbage
/// output if run against the wrong kind.
pub fn classify_chd_info(info_output: &str) -> &'static str {
    if info_output.contains("Tag='CHT2'") || info_output.contains("Tag='CHTR'") {
        "cd"
    } else if info_output.contains("Tag='DVD '") {
        "dvd"
    } else {
        "unknown"
    }
}

#[cfg(test)]
mod classify_tests {
    use super::classify_chd_info;

    // Captured verbatim from `chdman info` (0.289) against a real CD-type
    // CHD produced by `createcd`.
    const CD_INFO: &str = "chdman - MAME Compressed Hunks of Data (CHD) manager 0.289 (mame0289)\nInput file:   test_cd.chd\nFile Version: 5\nLogical size: 244,800 bytes\nHunk Size:    19,584 bytes\nTotal Hunks:  13\nUnit Size:    2,448 bytes\nTotal Units:  100\nCompression:  cdlz (CD LZMA), cdzl (CD Deflate), cdfl (CD FLAC)\nCHD size:     293 bytes\nRatio:        0.1%\nSHA1:         1135febcf39f68f8ee90df246c0bb0f31517f53c\nData SHA1:    0288a92a2ce4f642d981dbe146a897dc31e659ac\nMetadata:     Tag='CHT2'  Index=0  Length=86 bytes\n              TRACK:1 TYPE:MODE1 SUBTYPE:NONE FRAMES:100 PREGAP:0 PGTYPE:M\n";

    // Captured verbatim from `chdman info` against a real DVD-type CHD
    // produced by `createdvd -c zlib`.
    const DVD_INFO: &str = "chdman - MAME Compressed Hunks of Data (CHD) manager 0.289 (mame0289)\nInput file:   test_dvd.chd\nFile Version: 5\nLogical size: 204,800 bytes\nHunk Size:    4,096 bytes\nTotal Hunks:  50\nUnit Size:    2,048 bytes\nTotal Units:  100\nCompression:  zlib (Deflate)\nCHD size:     188 bytes\nRatio:        0.1%\nSHA1:         8f206ee70c071466ee32ab91976cd519e5ae169d\nData SHA1:    e125b3b7525bd2a65d9bc7591f6cfedaf57a034a\nMetadata:     Tag='DVD '  Index=0  Length=1 bytes\n              .\n";

    const MALFORMED_INFO: &str = "chdman - MAME Compressed Hunks of Data (CHD) manager 0.289 (mame0289)\nError opening file: NOT A CHD\n";

    #[test]
    fn detects_cd_from_track_metadata_tag() {
        assert_eq!(classify_chd_info(CD_INFO), "cd");
    }

    #[test]
    fn detects_dvd_from_dvd_metadata_tag() {
        assert_eq!(classify_chd_info(DVD_INFO), "dvd");
    }

    #[test]
    fn reports_unknown_for_unrecognized_output() {
        assert_eq!(classify_chd_info(MALFORMED_INFO), "unknown");
    }

    #[test]
    fn reports_unknown_for_empty_output() {
        assert_eq!(classify_chd_info(""), "unknown");
    }
}
