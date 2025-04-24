// tests for dialog of impl item

use super::*;

const PROMPT: &str = "Found 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>' of required 'MyMap2D (Struct)'.";
const HELP: &str = "↑↓ to move, enter to select, type to filter, and esc to quit.";

static OPTIONS: Lazy<Vec<String>> = Lazy::new(|| {
    vec![
        String::from(
            "Include 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>'.",
        ),
        String::from(
            "Exclude 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>'.",
        ),
        String::from(
            "Include all items of 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>'.",
        ),
        String::from(
            "Show code of 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>'.",
        ),
    ]
});

fn prepare_test() -> (
    CgData<FusionCli, ProcessingImplItemDialogState>,
    NodeIndex,
    NodeIndex,
) {
    // preparation
    let cg_data = setup_processing_test(false)
        .add_challenge_dependencies()
        .unwrap()
        .add_src_files()
        .unwrap()
        .expand_use_statements()
        .unwrap()
        .path_minimizing_of_use_and_path_statements()
        .unwrap()
        .link_impl_blocks_with_corresponding_item()
        .unwrap()
        .link_required_by_challenge()
        .unwrap();

    // get impl block index
    let my_map_2d_impl_block_index = cg_data
        .iter_crates()
        .flat_map(|(n, _, _)| cg_data.iter_syn_items(n))
        .find_map(|(n, i)| if let ItemName::ImplBlockIdentifier(name) = ItemName::from(i) {
            (name == "impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>").then_some(n)
        } else {
            None
        })
        .unwrap();
    let struct_my_map_2d_node = cg_data
        .iter_crates()
        .flat_map(|(n, _, _)| cg_data.iter_syn_items(n))
        .filter(|(_, i)| matches!(i, Item::Struct(_)))
        .find_map(|(n, i)| {
            if let Some(name) = ItemName::from(i).get_ident_in_name_space() {
                (name == "MyMap2D").then_some(n)
            } else {
                None
            }
        })
        .unwrap();
    (cg_data, my_map_2d_impl_block_index, struct_my_map_2d_node)
}

#[test]
fn test_impl_block_selection() {
    // preparation
    let (cg_data, my_map_2d_impl_block_index, struct_my_map_2d_node) = prepare_test();

    // prepare mock for include
    let mut mock = TestCgDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[0].to_owned())));

    // prepare mock for exclude
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[1].to_owned())));

    // prepare mock for include block items
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[2].to_owned())));

    // prepare mock for exclude block items
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[3].to_owned())));

    // prepare mock for use quits
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(None));

    // prepare mock for show usage of item
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some("Some bad output".into())));

    // test and assert
    // include
    let test_result = cg_data
        .impl_block_selection(my_map_2d_impl_block_index, struct_my_map_2d_node, &mut mock)
        .unwrap();
    assert_eq!(test_result, DialogImplBlockSelection::IncludeImplBlock);

    // exclude
    let test_result = cg_data
        .impl_block_selection(my_map_2d_impl_block_index, struct_my_map_2d_node, &mut mock)
        .unwrap();
    assert_eq!(test_result, DialogImplBlockSelection::ExcludeImplBlock);

    // include block items
    let test_result = cg_data
        .impl_block_selection(my_map_2d_impl_block_index, struct_my_map_2d_node, &mut mock)
        .unwrap();
    assert_eq!(
        test_result,
        DialogImplBlockSelection::IncludeAllItemsOfImplBlock
    );

    // exclude block items
    let test_result = cg_data
        .impl_block_selection(my_map_2d_impl_block_index, struct_my_map_2d_node, &mut mock)
        .unwrap();
    assert_eq!(test_result, DialogImplBlockSelection::ShowImplBlock);

    // user quits
    let test_result = cg_data
        .impl_block_selection(my_map_2d_impl_block_index, struct_my_map_2d_node, &mut mock)
        .unwrap();
    assert_eq!(test_result, DialogImplBlockSelection::Quit);

    // bad output
    let test_result = cg_data
        .impl_block_selection(my_map_2d_impl_block_index, struct_my_map_2d_node, &mut mock)
        .unwrap();
    assert_eq!(test_result, DialogImplBlockSelection::Quit);
}

#[test]
fn test_impl_block_dialog_include() {
    // preparation
    let (cg_data, my_map_2d_impl_block_index, struct_my_map_2d_node) = prepare_test();

    // prepare mock for include
    let mut mock = TestCgDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[0].to_owned())));

    // assert
    let test_result = cg_data
        .impl_block_dialog(my_map_2d_impl_block_index, struct_my_map_2d_node, &mut mock)
        .unwrap();

    assert_eq!(test_result, vec![(my_map_2d_impl_block_index, true)]);
}

#[test]
fn test_impl_block_dialog_exclude() {
    // preparation
    let (cg_data, my_map_2d_impl_block_index, struct_my_map_2d_node) = prepare_test();

    // prepare mock for exclude
    let mut mock = TestCgDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[1].to_owned())));

    // assert
    let test_result = cg_data
        .impl_block_dialog(my_map_2d_impl_block_index, struct_my_map_2d_node, &mut mock)
        .unwrap();

    assert_eq!(test_result, vec![(my_map_2d_impl_block_index, false)]);
}

#[test]
fn test_impl_block_dialog_include_block_items() {
    // preparation
    let (cg_data, my_map_2d_impl_block_index, struct_my_map_2d_node) = prepare_test();

    // prepare mock for include all block items
    let mut mock = TestCgDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[2].to_owned())));

    // assert
    let test_result = cg_data
        .impl_block_dialog(my_map_2d_impl_block_index, struct_my_map_2d_node, &mut mock)
        .unwrap();

    let expected_result: Vec<(NodeIndex, bool)> = cg_data
        .iter_syn_impl_item(my_map_2d_impl_block_index)
        .filter_map(|(n, _)| (!cg_data.is_required_by_challenge(n)).then_some((n, true)))
        .collect();

    assert_eq!(test_result, expected_result);
}

#[test]
fn test_impl_block_dialog_show_block_and_include() {
    // preparation
    let (cg_data, my_map_2d_impl_block_index, struct_my_map_2d_node) = prepare_test();

    // prepare mock for include
    let mut mock = TestCgDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[3].to_owned())));
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[0].to_owned())));

    // assert
    let test_result = cg_data
        .impl_block_dialog(my_map_2d_impl_block_index, struct_my_map_2d_node, &mut mock)
        .unwrap();

    assert_eq!(test_result, vec![(my_map_2d_impl_block_index, true)]);
    let writer_content = String::from_utf8(mock.dialog.writer.into_inner()).unwrap();
    assert_eq!(
        writer_content,
        r#"
/home/marc/Development/repos/codingame/create_single_code_file/cg_fusion_lib_test/my_map_two_dim/src/lib.rs:19:1
impl<T: Copy + Clone + Default, const X: usize, const Y: usize, const N: usize>
    MyMap2D<T, X, Y, N>
{
    pub fn new() -> Self {
        if X == 0 {
            panic!("line {}, minimum one column", line!());
        }
        if Y == 0 {
            panic!("line {}, minimum one row", line!());
        }
        Self {
            items: [[T::default(); X]; Y],
        }
    }
    pub fn init(init_element: T) -> Self {
        if X == 0 {
            panic!("line {}, minimum one column", line!());
        }
        if Y == 0 {
            panic!("line {}, minimum one row", line!());
        }
        Self {
            items: [[init_element; X]; Y],
        }
    }
    pub fn get(&self, coordinates: MapPoint<X, Y>) -> &T {
        &self.items[coordinates.y()][coordinates.x()]
    }
    pub fn get_mut(&mut self, coordinates: MapPoint<X, Y>) -> &mut T {
        &mut self.items[coordinates.y()][coordinates.x()]
    }
    pub fn set(&mut self, coordinates: MapPoint<X, Y>, value: T) -> &T {
        self.items[coordinates.y()][coordinates.x()] = value;
        &self.items[coordinates.y()][coordinates.x()]
    }
    pub fn is_cut_off_cell(
        &self,
        map_point: MapPoint<X, Y>,
        is_cell_free_fn: IsCellFreeFn<X, Y, T>,
    ) -> bool {
        let (mut last_free, initial_orientation) = match map_point.map_position() {
            Compass::NW | Compass::N => (false, Compass::E),
            Compass::NE | Compass::E => (false, Compass::S),
            Compass::SE | Compass::S => (false, Compass::W),
            Compass::SW | Compass::W => (false, Compass::N),
            Compass::Center => {
                let nw = map_point.neighbor(Compass::NW).unwrap();
                (is_cell_free_fn(nw, self.get(nw)), Compass::N)
            }
        };
        let mut free_zones = 0;
        for (is_free, is_side) in map_point
            .iter_neighbors(initial_orientation, true, false, true)
            .map(|(p, o)| (is_cell_free_fn(p, self.get(p)), o.is_cardinal()))
        {
            if !last_free && is_free && is_side {
                // new free zones start always at a side of map_point, since movement over corners is not allowed
                free_zones += 1;
            }
            last_free = if is_side || !is_free {
                // side or blocked corner -> apply is_free to last_free
                is_free
            } else {
                // free corner -> keep old value of last_free
                last_free
            };
        }
        free_zones > 1
    }
    pub fn iter(&self) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        self.items.iter().enumerate().flat_map(|(y, row)| {
            row.iter()
                .enumerate()
                .map(move |(x, column)| (MapPoint::<X, Y>::new(x, y), column))
        })
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (MapPoint<X, Y>, &mut T)> {
        self.items.iter_mut().enumerate().flat_map(|(y, row)| {
            row.iter_mut()
                .enumerate()
                .map(move |(x, column)| (MapPoint::<X, Y>::new(x, y), column))
        })
    }
    pub fn iter_row(&self, r: usize) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        if r >= Y {
            panic!("line {}, row index is out of range", line!());
        }
        self.items
            .iter()
            .enumerate()
            .filter(move |(y, _)| *y == r)
            .flat_map(|(y, row)| {
                row.iter()
                    .enumerate()
                    .map(move |(x, column)| (MapPoint::new(x, y), column))
            })
    }
    pub fn iter_column(&self, c: usize) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        if c >= X {
            panic!("line {}, column index is out of range", line!());
        }
        self.items.iter().enumerate().flat_map(move |(y, row)| {
            row.iter()
                .enumerate()
                .filter(move |(x, _)| *x == c)
                .map(move |(x, column)| (MapPoint::new(x, y), column))
        })
    }
    pub fn iter_neighbors(
        &self,
        center_point: MapPoint<X, Y>,
    ) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        center_point
            .iter_neighbors(Compass::N, true, false, false)
            .map(move |(p, _)| (p, self.get(p)))
    }
    pub fn iter_neighbors_mut(
        &mut self,
        center_point: MapPoint<X, Y>,
    ) -> impl Iterator<Item = (MapPoint<X, Y>, &mut T)> {
        center_point
            .iter_neighbors(Compass::N, true, false, false)
            .map(move |(p, _)| unsafe { (p, &mut *(self.get_mut(p) as *mut _)) })
    }
    pub fn iter_neighbors_with_center(
        &self,
        center_point: MapPoint<X, Y>,
    ) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        center_point
            .iter_neighbors(Compass::N, true, true, false)
            .map(move |(p, _)| (p, self.get(p)))
    }
    pub fn iter_neighbors_with_corners(
        &self,
        center_point: MapPoint<X, Y>,
    ) -> impl Iterator<Item = (MapPoint<X, Y>, &T, bool)> {
        center_point
            .iter_neighbors(Compass::N, true, false, true)
            .map(move |(p, o)| (p, self.get(p), o.is_ordinal()))
    }
    pub fn iter_neighbors_with_center_and_corners(
        &self,
        center_point: MapPoint<X, Y>,
    ) -> impl Iterator<Item = (MapPoint<X, Y>, &T, bool)> {
        center_point
            .iter_neighbors(Compass::N, true, true, true)
            .map(move |(p, o)| (p, self.get(p), o.is_ordinal()))
    }
    pub fn iter_orientation(
        &self,
        start_point: MapPoint<X, Y>,
        orientation: Compass,
    ) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        start_point
            .iter_orientation(orientation)
            .map(move |p| (p, self.get(p)))
    }
    pub fn iter_diagonal_top_left(&self) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        MapPoint::<X, Y>::new(0, 0)
            .iter_orientation(Compass::SE)
            .map(move |p| (p, self.get(p)))
    }
    pub fn iter_diagonal_top_right(&self) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        MapPoint::<X, Y>::new(X - 1, 0)
            .iter_orientation(Compass::SW)
            .map(move |p| (p, self.get(p)))
    }
    pub fn iter_diagonal_bottom_left(&self) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        MapPoint::<X, Y>::new(0, Y - 1)
            .iter_orientation(Compass::NE)
            .map(move |p| (p, self.get(p)))
    }
    pub fn iter_diagonal_bottom_right(&self) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        MapPoint::<X, Y>::new(X - 1, Y - 1)
            .iter_orientation(Compass::NW)
            .map(move |p| (p, self.get(p)))
    }
    pub fn iter_distance(
        &self,
        start_point: MapPoint<X, Y>,
        filter_fn: FilterFn<X, Y, T>,
    ) -> impl Iterator<Item = (MapPoint<X, Y>, &'_ T, usize)> {
        // use filter_fn as follows (use "_" for unused variables):
        // let filter_fn = Box::new(|point_of_next_cell: MapPoint<X, Y>, value_of_next_cell: &T, current_distance: usize| current_point.use_it_somehow() || current_cell_value.use_it_somehow() || current_distance.use_it_somehow());
        DistanceIter::new(self, start_point, filter_fn)
    }
}

"#
    );
}
