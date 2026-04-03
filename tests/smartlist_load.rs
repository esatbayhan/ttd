use std::fs;
use std::path::PathBuf;
use ttd::smartlist::load_all;

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("ttd-smartlist-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&path);
    path
}

#[test]
fn loads_and_sorts_list_files_by_order_then_name() {
    let dir = temp_path("sort_by_order");
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("three.list"),
        "---\nname: Three\norder: 3\n---\n",
    )
    .unwrap();
    fs::write(
        dir.join("one.list"),
        "---\nname: One\norder: 1\n---\n",
    )
    .unwrap();
    fs::write(
        dir.join("two.list"),
        "---\nname: Two\norder: 2\n---\n",
    )
    .unwrap();

    let lists = load_all(&dir);
    fs::remove_dir_all(&dir).unwrap();

    assert_eq!(lists.len(), 3);
    assert_eq!(lists[0].name, "One");
    assert_eq!(lists[0].order, Some(1));
    assert_eq!(lists[1].name, "Two");
    assert_eq!(lists[1].order, Some(2));
    assert_eq!(lists[2].name, "Three");
    assert_eq!(lists[2].order, Some(3));
}

#[test]
fn lists_without_order_sort_after_ordered_lists_alphabetically() {
    let dir = temp_path("sort_unordered");
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("beta.list"),
        "---\nname: Beta\n---\n",
    )
    .unwrap();
    fs::write(
        dir.join("alpha.list"),
        "---\nname: Alpha\n---\n",
    )
    .unwrap();
    fs::write(
        dir.join("first.list"),
        "---\nname: First\norder: 1\n---\n",
    )
    .unwrap();

    let lists = load_all(&dir);
    fs::remove_dir_all(&dir).unwrap();

    assert_eq!(lists.len(), 3);
    assert_eq!(lists[0].name, "First");
    assert_eq!(lists[0].order, Some(1));
    assert_eq!(lists[1].name, "Alpha");
    assert_eq!(lists[1].order, None);
    assert_eq!(lists[2].name, "Beta");
    assert_eq!(lists[2].order, None);
}

#[test]
fn returns_empty_vec_when_lists_dir_does_not_exist() {
    let dir = temp_path("nonexistent");
    // Ensure the directory doesn't exist
    let _ = fs::remove_dir_all(&dir);

    let lists = load_all(&dir);

    assert!(lists.is_empty());
}

#[test]
fn skips_non_list_files() {
    let dir = temp_path("skip_non_list");
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("valid.list"),
        "---\nname: Valid\norder: 1\n---\n",
    )
    .unwrap();
    fs::write(dir.join("readme.txt"), "this is a text file").unwrap();
    fs::write(dir.join("notes.md"), "# markdown notes").unwrap();

    let lists = load_all(&dir);
    fs::remove_dir_all(&dir).unwrap();

    assert_eq!(lists.len(), 1);
    assert_eq!(lists[0].name, "Valid");
}

#[test]
fn malformed_file_included_with_parse_error() {
    let dir = temp_path("malformed");
    fs::create_dir_all(&dir).unwrap();

    // File without frontmatter delimiters
    fs::write(
        dir.join("broken.list"),
        "not done\ndue < today\n",
    )
    .unwrap();

    let lists = load_all(&dir);
    fs::remove_dir_all(&dir).unwrap();

    assert_eq!(lists.len(), 1);
    assert!(lists[0].parse_error.is_some());
}
