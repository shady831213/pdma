use super::support::{TestResult, compile_fragments, render_map};
use hw_model_schema::{Access, register_map, register_schema};

register_schema! { pub register Word { width: 4, endian: Little, access: ReadOnly,
reset: Zero(0), writable_mask: 0, fields: {} } }
register_map! { pub map Leaf { alignment: 4, placements: [value: Word] } }
register_map! { pub map One { alignment: 4, placements: [left: Leaf] } }
register_map! { pub map More { alignment: 4, placements: [left: Leaf, right: Leaf, other: Word] } }

#[test]
fn ancestors_and_existing_names_survive_added_same_leaf_siblings() -> TestResult {
    // Given: a nested register and a larger map adding same-leaf and unrelated siblings.
    let one = One::resolve()?;
    let more = More::resolve()?;
    // When: both maps render with the same explicit root prefix.
    let artifacts = [
        render_map("one", "root", &one)?,
        render_map("more", "root", &more)?,
    ];
    // Then: the original structural name remains unchanged and native C observes its address.
    for artifact in &artifacts {
        let text = std::str::from_utf8(&artifact.contents)?;
        assert!(
            text.contains("C_ROOT_C_LEFT_C_VALUE_OFFSET UINT64_C(0)"),
            "{text}"
        );
        compile_fragments(
            "direct-ancestor",
            &[&artifact.contents],
            b"int main(void) { return C_ROOT_C_LEFT_C_VALUE_OFFSET != 0; }\n",
        )?;
    }
    Ok(())
}

#[test]
fn projected_subset_preserves_surviving_register_names() -> TestResult {
    // Given: a nested map and a projection retaining only its second leaf.
    let full = More::resolve()?;
    let subset =
        full.project_access(|register| (register.offset() == 4).then_some(Access::ReadOnly))?;
    // When: both views render under one root prefix.
    let artifacts = [
        render_map("full", "root", &full)?,
        render_map("subset", "root", &subset)?,
    ];
    // Then: removing the first same-leaf sibling does not rename the retained register.
    for artifact in &artifacts {
        let text = std::str::from_utf8(&artifact.contents)?;
        assert!(
            text.contains("C_ROOT_C_RIGHT_C_VALUE_OFFSET UINT64_C(4)"),
            "{text}"
        );
        compile_fragments(
            "direct-subset",
            &[&artifact.contents],
            b"int main(void) { return C_ROOT_C_RIGHT_C_VALUE_OFFSET != 4; }\n",
        )?;
    }
    Ok(())
}
