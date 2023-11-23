use crate::draw;
use anyhow::{anyhow, bail, Result};
use glib::timeout_future;
use gtk::{gdk::Texture, prelude::TextureExt};
use log::{debug, error};
use poppler::Document;
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    rc::Rc,
    time::{Duration, Instant},
};

pub type PageNumber = usize;
pub type MyPageType = Texture;

pub struct PageCache {
    document: Document,
    max_num_stored_pages: usize,
    pages: BTreeMap<usize, Rc<MyPageType>>,
    last_requested_page_number: PageNumber,
}

impl PageCache {
    pub fn new(document: Document, max_num_stored_pages: usize) -> Self {
        PageCache {
            document,
            max_num_stored_pages,
            pages: BTreeMap::new(),
            last_requested_page_number: 0,
        }
    }

    pub fn get_page(&mut self, page_number: usize) -> Option<Rc<MyPageType>> {
        self.last_requested_page_number = page_number;
        self.pages.get(&page_number).map(Rc::clone)
    }

    pub fn get_page_or_cache(&mut self, page_number: usize) -> Result<Rc<MyPageType>> {
        if let Some(page) = self.get_page(page_number) {
            return Ok(page);
        } else {
            let _ = self.cache_page(page_number, 100);
            if let Some(page) = self.get_page(page_number) {
                return Ok(page);
            } else {
                bail!("Failed caching and retrieving page {}", page_number);
            }
        }
    }

    pub fn cache_page(&mut self, page_number: PageNumber, height: i32) -> Option<CacheResponse> {
        debug!("Caching page {}", page_number);
        let begin_of_cashing = Instant::now();
        if let Some(page) = self.pages.get(&page_number) {
            if page.height() >= height {
                debug!("Page already in cache");
                return None;
            }
        }

        let mut response = None;

        if let Some(page) = self.document.page(page_number as i32) {
            let pages = vec![Rc::new(page)];
            let texture = draw::draw_pages_to_texture(&pages, height);
            let page = Rc::new(texture);

            // Overwrite page with lower resolution if exists
            let previous_page = self.pages.insert(page_number, Rc::clone(&page));
            let page_resolution_upgraded = previous_page.is_some();
            if page_resolution_upgraded {
                response = Some(CacheResponse::PageResolutionUpgraded { page_number, page });
            }

            if self.pages.len() > self.max_num_stored_pages && self.pages.len() > 2 {
                let _result = self.remove_most_distant_page();
            }
        }
        debug!(
            "done caching of page {} in {}ms",
            page_number,
            begin_of_cashing.elapsed().as_millis()
        );
        response
    }

    fn remove_most_distant_page(&mut self) -> anyhow::Result<()> {
        let (min_cached_page_number, min_cached_page) = self
            .pages
            .pop_first()
            .ok_or(anyhow!("The cache is empty, cannot remove first page"))?;
        let (max_cached_page_number, max_cached_page) = self
            .pages
            .pop_last()
            .ok_or(anyhow!("The cache is empty, cannot remove last page"))?;

        if self
            .last_requested_page_number
            .abs_diff(min_cached_page_number)
            > self
                .last_requested_page_number
                .abs_diff(max_cached_page_number)
        {
            self.pages.insert(max_cached_page_number, max_cached_page);
            debug!(
                "Removed page {} from cache to keep size low...",
                min_cached_page_number
            );
        } else {
            self.pages.insert(min_cached_page_number, min_cached_page);
            debug!(
                "Removed page {} from cache to keep size low...",
                max_cached_page_number
            );
        }

        Ok(())
    }

    fn process_command(&mut self, command: CacheCommand) -> Result<Option<CacheResponse>> {
        debug!("Processing command: {:?}...", command);
        match command {
            CacheCommand::Cache(command) => Ok(self.cache_page(command.page, command.height)),
            CacheCommand::Retrieve(command) => match command {
                RetrievePagesCommand::GetCurrentTwoPages { page_left_number } => {
                    let page_left = self.get_page_or_cache(page_left_number)?;
                    let page_right = self.get_page_or_cache(page_left_number + 1)?;
                    Ok(Some(CacheResponse::TwoPagesRetrieved {
                        page_left,
                        page_right,
                    }))
                }
                RetrievePagesCommand::GetCurrentPage { page_number } => {
                    let page = self.get_page_or_cache(page_number)?;
                    Ok(Some(CacheResponse::SinglePageRetrieved { page }))
                }
            },
        }
    }
}

#[derive(Debug)]
pub enum CacheCommand {
    Cache(CachePageCommand),
    Retrieve(RetrievePagesCommand),
}

#[derive(Debug)]
pub struct CachePageCommand {
    page: PageNumber,
    height: i32,
}

#[derive(Debug)]
pub enum RetrievePagesCommand {
    GetCurrentTwoPages { page_left_number: PageNumber },
    GetCurrentPage { page_number: PageNumber },
}

pub enum CacheResponse {
    SinglePageRetrieved {
        page: Rc<MyPageType>,
    },
    TwoPagesRetrieved {
        page_left: Rc<MyPageType>,
        page_right: Rc<MyPageType>,
    },
    PageResolutionUpgraded {
        page_number: PageNumber,
        page: Rc<MyPageType>,
    },
}

pub struct SyncCacheCommandChannel {
    retrieve_commands: Vec<RetrievePagesCommand>,
    cache_commands: VecDeque<CachePageCommand>,
}

pub struct SyncCacheCommandSender {
    channel: Rc<RefCell<SyncCacheCommandChannel>>,
}

pub struct SyncCacheCommandReceiver {
    channel: Rc<RefCell<SyncCacheCommandChannel>>,
}

impl SyncCacheCommandChannel {
    pub fn open() -> (SyncCacheCommandSender, SyncCacheCommandReceiver) {
        let channel = SyncCacheCommandChannel {
            retrieve_commands: Vec::new(),
            cache_commands: VecDeque::new(),
        };
        let channel = Rc::new(RefCell::new(channel));

        let sender = SyncCacheCommandSender {
            channel: Rc::clone(&channel),
        };
        let receiver = SyncCacheCommandReceiver { channel };
        (sender, receiver)
    }
}

impl SyncCacheCommandSender {
    pub fn is_channel_open(&self) -> bool {
        Rc::strong_count(&self.channel) > 1
    }

    pub fn send_retrieve_command(&self, command: RetrievePagesCommand) {
        // Make newest message the most important
        self.channel.borrow_mut().retrieve_commands.push(command);
    }

    pub fn send_cache_commands(&self, pages: &[PageNumber], height: i32) {
        for &page in pages {
            // Make message in front the most important
            self.channel
                .borrow_mut()
                .cache_commands
                .push_front(CachePageCommand { page, height: 10 }); // Cache with lower resolution
            self.channel
                .borrow_mut()
                .cache_commands
                .push_back(CachePageCommand { page, height });
        }
    }
}

impl SyncCacheCommandReceiver {
    pub fn is_channel_open(&self) -> bool {
        Rc::strong_count(&self.channel) > 1
    }

    pub fn receive_most_important_command(&self) -> Option<CacheCommand> {
        let mut channel = self.channel.borrow_mut();
        if let Some(command) = channel.retrieve_commands.pop() {
            return Some(CacheCommand::Retrieve(command));
        } else if let Some(command) = channel.cache_commands.pop_front() {
            return Some(CacheCommand::Cache(command));
        }
        None
    }
}

pub fn spawn_sync_cache<F>(document: Document, receiver: F) -> SyncCacheCommandSender
where
    F: Fn(CacheResponse) + 'static,
{
    let (command_sender, command_receiver) = SyncCacheCommandChannel::open();

    let mut cache = PageCache::new(document, 20);

    // Besides the name, it is not in another thread
    glib::spawn_future_local(async move {
        while command_receiver.is_channel_open() {
            // Add delay to tell gtk to give rendering priority
            timeout_future(Duration::from_millis(1)).await;

            if let Some(command) = command_receiver.receive_most_important_command() {
                if let Some(response) = cache.process_command(command).unwrap_or_else(|e| {
                    error!("Error processing command: {}", e);
                    None
                }) {
                    // response_sender.send_blocking(response).unwrap();
                    debug!("Command processed, activating receiver....");
                    receiver(response);
                    debug!("receiver done");
                }
            }
        }
    });

    command_sender
}
