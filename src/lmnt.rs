#![allow(unused)]

use xmltree::{Element, XMLNode};

pub trait LMNT {
    fn find_first_child(&self, tag: &str) -> Option<&Element>;
    fn find_first_child_with_attrs(&self, tag: &str, attrs: &[(&str, &str)]) -> Option<&Element>;
    fn find_first_child_with_attrs_mut(
        &mut self,
        tag: &str,
        attrs: &[(&str, &str)],
    ) -> Option<&mut Element>;
    fn descendants(&self) -> Descendants;
}

impl LMNT for Element {
    /// Finds first child element with matching tag name
    fn find_first_child(&self, tag: &str) -> Option<&Element> {
        for c in &self.children {
            match c {
                XMLNode::Element(element) => {
                    if element.name == tag {
                        return Some(element);
                    } else {
                        match element.find_first_child(tag) {
                            Some(e) => return Some(e),
                            None => continue,
                        }
                    }
                }
                _ => continue,
            }
        }
        return None;
    }

    /// Finds first descendant element with matching tag name that also
    /// contains the provided attribute (key, value) pairs
    fn find_first_child_with_attrs(&self, tag: &str, attrs: &[(&str, &str)]) -> Option<&Element> {
        for c in &self.children {
            match c {
                XMLNode::Element(element) => {
                    if element.name == tag
                        && attrs
                            .iter()
                            .all(|(k, v)| element.attributes.get(*k).is_some_and(|val| val == v))
                    {
                        return Some(element);
                    } else {
                        match element.find_first_child_with_attrs(tag, attrs) {
                            Some(e) => return Some(e),
                            None => continue,
                        }
                    }
                }
                _ => continue,
            }
        }

        return None;
    }

    fn find_first_child_with_attrs_mut(
        &mut self,
        tag: &str,
        attrs: &[(&str, &str)],
    ) -> Option<&mut Element> {
        for c in self.children.iter_mut() {
            match c {
                XMLNode::Element(element) => {
                    if element.name == tag
                        && attrs
                            .iter()
                            .all(|(k, v)| element.attributes.get(*k).is_some_and(|val| val == v))
                    {
                        return Some(element);
                    } else {
                        match element.find_first_child_with_attrs_mut(tag, attrs) {
                            Some(e) => return Some(e),
                            None => continue,
                        }
                    }
                }
                _ => continue,
            }
        }

        return None;
    }

    /// Creates an iterator that returns child Elements by searching depth-first
    ///
    /// Example:
    ///
    /// <root id='root'>
    ///	  <child id='c1'>
    ///		<grandchild id='c1-gc1'></grandchild>
    ///		<grandchild id='c1-gc2'></grandchild>
    ///	  </child>
    ///	  <child id='c2'></child>
    /// </root>"
    ///
    /// Elements will be returned in order:
    /// root, c1, c1-gc1, c1-gc2, c2
    ///
    fn descendants(&self) -> Descendants {
        return Descendants::new(self);
    }
}

pub struct Descendants<'a> {
    stack: Vec<&'a Element>,
}

impl<'a> Descendants<'a> {
    fn new(root: &'a Element) -> Self {
        Self { stack: vec![root] }
    }
}

impl<'a> Iterator for Descendants<'a> {
    type Item = &'a Element;
    fn next(&mut self) -> Option<Self::Item> {
        let c = self.stack.pop()?;

        for child in c.children.iter().filter_map(|x| x.as_element()).rev() {
            self.stack.push(child);
        }

        return Some(c);
    }
}

mod test {
    use xmltree::Element;

    use super::LMNT;

    const TEST_XML: &str = r"<root id='root'>
	<child id='c1'>
		<grandchild id='c1-gc1'></grandchild>
		<grandchild id='c1-gc2'></grandchild>
	</child>
	<child id='c2'>Hi</child>
	<child id='c3'>
		<grandchild id='c3-gc1'></grandchild>
		<grandchild id='c3-gc2'>
			<greatgrandchild id='c3-gc2-ggc1'></greatgrandchild>
		</grandchild>
	</child>
</root>";

    #[test]
    fn test_iter() {
        const ORDER: [&str; 9] = [
            "root",
            "c1",
            "c1-gc1",
            "c1-gc2",
            "c2",
            "c3",
            "c3-gc1",
            "c3-gc2",
            "c3-gc2-ggc1",
        ];

        let mut root = Element::parse(TEST_XML.as_bytes()).unwrap();
        for (i, d) in root.descendants().enumerate() {
            let id = &d.attributes["id"];
            assert_eq!(id, ORDER[i])
        }
    }
}
