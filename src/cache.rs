use cairo::ImageSurface;
use glib::{timeout_future, timeout_future_seconds, ControlFlow};
use gtk::gdk::Texture;
use poppler::{Document, Page};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};

use async_channel::Sender;

use crate::draw;

type PageNumber = usize;
pub type MyPageType = Texture;

pub struct PageCache {
    document: Document,
    max_num_stored_pages: usize,
    pages: BTreeMap<usize, Rc<MyPageType>>,
}

impl PageCache {
    pub fn new(document: Document, max_num_stored_pages: usize) -> Self {
        PageCache {
            document,
            max_num_stored_pages,
            pages: BTreeMap::new(),
        }
    }

    pub fn get_page(&self, page_number: usize) -> Option<Rc<MyPageType>> {
        self.pages.get(&page_number).map(Rc::clone)
    }

    pub fn cache_pages(&mut self, page_numbers: Vec<usize>, area_height: i32) {
        println!("Caching pages {:?}", page_numbers);
        let begin_of_cashing = Instant::now();
        for page_number in page_numbers {
            if self.pages.contains_key(&page_number) {
                continue;
            }

            if let Some(page) = self.document.page(page_number as i32) {
                let pages = vec![Rc::new(page)];
                let texture = draw::draw_pages_to_texture(&pages, area_height);

                self.pages.insert(page_number, Rc::new(texture));

                if self.pages.len() > self.max_num_stored_pages && self.pages.len() > 2 {
                    let _result = self.remove_most_distant_page(page_number);
                }
            }
        }
        println!(
            "done caching in {}ms",
            begin_of_cashing.elapsed().as_millis()
        );
    }

    fn remove_most_distant_page(&mut self, current_page_number: usize) -> Result<(), ()> {
        let (min_cached_page_number, min_cached_page) = self.pages.pop_first().ok_or(())?;
        let (max_cached_page_number, max_cached_page) = self.pages.pop_last().ok_or(())?;

        if current_page_number.abs_diff(min_cached_page_number)
            > current_page_number.abs_diff(max_cached_page_number)
        {
            self.pages.insert(max_cached_page_number, max_cached_page);
        } else {
            self.pages.insert(min_cached_page_number, min_cached_page);
        }

        Ok(())
    }

    fn process_command(&mut self, command: CacheCommand) -> Option<CacheResponse> {
        println!("Processing command: {:?}...", command);
        match command {
            CacheCommand::CachePages { pages, area_height } => {
                self.cache_pages(pages, area_height);
                None
            }
            CacheCommand::GetCurrentTwoPages { page_left_number } => {
                if let Some(page_left) = self.get_page(page_left_number) {
                    println!("got left page");
                    if let Some(page_right) = self.get_page(page_left_number + 1) {
                        println!("got right page");
                        Some(CacheResponse::TwoPagesRetrieved {
                            page_left,
                            page_right,
                        })
                    } else {
                        Some(CacheResponse::SinglePageRetrieved { page: page_left })
                    }
                } else {
                    println!("did not get any page");
                    // TODO: if page left was not empty, this could be because page turning was too quick.
                    // In this case, just not rendering the current page is okay, but when no more render requests are available, one would want to wait for the caching
                    None
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum CacheCommand {
    CachePages {
        pages: Vec<PageNumber>,
        area_height: i32,
    },
    GetCurrentTwoPages {
        page_left_number: PageNumber,
    },
}

pub enum CacheResponse {
    SinglePageRetrieved {
        page: Rc<MyPageType>,
    },
    TwoPagesRetrieved {
        page_left: Rc<MyPageType>,
        page_right: Rc<MyPageType>,
    },
}

pub fn spawn_async_cache<F>(document: Document, receiver: F) -> Sender<CacheCommand>
where
    F: Fn(CacheResponse) + 'static,
{
    let (command_sender, command_receiver) = async_channel::unbounded();

    let mut cache = PageCache::new(document, 10);

    glib::spawn_future_local(async move {
        while let Ok(command) = command_receiver.recv().await {
            if let Some(response) = cache.process_command(command) {
                // response_sender.send_blocking(response).unwrap();
                println!("Command processed, activating receiver....");
                receiver(response);
                println!("receiver done");
            }

            // Add delay to tell gtk to give rendering priority
            timeout_future(Duration::from_millis(1)).await;
        }
    });

    command_sender
}
