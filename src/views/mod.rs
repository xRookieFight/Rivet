//! Screens. Each one implements `Page` and is built fresh from a `Route`.

pub mod common;
pub mod dashboard;
pub mod feed;
pub mod local;
pub mod login;
pub mod my_items;
pub mod new_repo;
pub mod notifications;
pub mod profile;
pub mod repo;
pub mod search;
pub mod settings;

use crate::app::Cx;
use crate::route::Route;
use egui::Ui;

pub trait Page {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx);
}

pub fn make(route: &Route) -> Box<dyn Page> {
    match route {
        Route::Dashboard => Box::new(dashboard::Dashboard::default()),
        Route::Feed => Box::new(dashboard::FeedPage::default()),
        Route::Notifications => Box::new(notifications::NotificationsPage::default()),
        Route::Search { q, kind } => Box::new(search::SearchPage::new(q.clone(), *kind)),
        Route::MyItems { pulls } => Box::new(my_items::MyItems::new(*pulls)),
        Route::User(login) => Box::new(profile::Profile::new(login.clone())),
        Route::NewRepo => Box::new(new_repo::NewRepo::default()),
        Route::Repo(r) => Box::new(repo::RepoPage::new(r.clone())),
        Route::Local(path) => Box::new(local::LocalPage::new(path.clone())),
        Route::Clone { url } => Box::new(local::ClonePage::new(url.clone())),
        Route::Settings => Box::new(settings::SettingsPage::default()),
    }
}
