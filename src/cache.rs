use glib::Bytes;
use gtk::gdk::Texture;
use pdfium_render::{pdfium, prelude::*};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use async_channel::{Receiver, Sender};

type PageNumber = usize;

pub struct PageCache<'a> {
    document: PdfDocument<'a>,
    render_config: PdfRenderConfig,
    max_num_stored_pages: usize,
    pages: BTreeMap<usize, Arc<Texture>>,
}

impl<'a> PageCache<'a> {
    pub fn new(
        document: PdfDocument<'a>,
        render_config: PdfRenderConfig,
        max_num_stored_pages: usize,
    ) -> Self {
        PageCache {
            document,
            render_config,
            max_num_stored_pages,
            pages: BTreeMap::new(),
        }
    }

    pub fn get_page(&self, page_number: usize) -> Option<Arc<Texture>> {
        self.pages.get(&page_number).map(Arc::clone)
    }

    pub fn cache_pages(&mut self, page_numbers: Vec<usize>) {
        for page_number in page_numbers {
            if self.pages.contains_key(&page_number) {
                continue;
            }

            let page = self.document.pages().get(page_number as u16).unwrap();
            let image = page.render_with_config(&self.render_config).unwrap();

            // TODO: does this clone?
            let bytes = Bytes::from(image.as_bytes());
            let page = Texture::from_bytes(&bytes).unwrap();
            // let page = self.document.page(page_number as i32);
            self.pages.insert(page_number, Arc::new(page));

            if self.pages.len() > self.max_num_stored_pages && self.pages.len() > 2 {
                let _result = self.remove_most_distant_page(page_number);
            }
        }
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
        match command {
            CacheCommand::CachePages { pages } => {
                self.cache_pages(pages);
                None
            }
            CacheCommand::GetCurrentTwoPages { page_left_number } => {
                let page_left = self
                    .get_page(page_left_number)
                    .expect("Requested page was not cached!!!");

                if let Some(page_right) = self.get_page(page_left_number + 1) {
                    Some(CacheResponse::TwoPagesLoaded {
                        page_left,
                        page_right,
                    })
                } else {
                    Some(CacheResponse::SinglePageLoaded { page: page_left })
                }
            }
        }
    }
}

pub enum CacheCommand {
    CachePages { pages: Vec<PageNumber> },
    GetCurrentTwoPages { page_left_number: PageNumber },
}

pub enum CacheResponse {
    DocumentLoaded {
        num_pages: usize,
    },
    SinglePageLoaded {
        page: Arc<Texture>,
    },
    TwoPagesLoaded {
        page_left: Arc<Texture>,
        page_right: Arc<Texture>,
    },
}

pub fn spawn_async_cache(
    file: impl AsRef<Path>,
) -> (Sender<CacheCommand>, Receiver<CacheResponse>) {
    let (command_sender, command_receiver) = async_channel::unbounded();
    let (response_sender, response_receiver) = async_channel::unbounded();

    let path: PathBuf = file.as_ref().to_path_buf();

    std::thread::spawn(move || {
        // Load pdf document here since Document is not thread safe and cannot be passed from main thread
        let pdfium = Pdfium::default();

        let document = pdfium.load_pdf_from_file(&path, None).unwrap();
        let render_config = PdfRenderConfig::new()
            .set_target_width(2000)
            .set_maximum_height(2000)
            .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);
        let num_pages = document.pages().iter().count();

        // let document = poppler::Document::from_file(&uri, None).unwrap();
        // let num_pages = document.n_pages() as usize;
        response_sender.send(CacheResponse::DocumentLoaded { num_pages });

        let mut cache = PageCache::new(document, render_config, 10);

        loop {
            if let Ok(command) = command_receiver.recv_blocking() {
                // if !command_receiver.is_empty() {
                //     // ignore command if more up to date ones are available
                //     continue;
                // }
                if let Some(response) = cache.process_command(command) {
                    response_sender.send_blocking(response);
                }
            } else {
                // Sender was closed, cache not needed anymore
                break;
            }
        }
    });

    (command_sender, response_receiver)
}
