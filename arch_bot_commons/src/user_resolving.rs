use teloxide::{
    requests::Requester,
    types::{Chat, Message, Update, UpdateKind, User, UserId},
    Bot, RequestError,
};

/// Reads a message's text, and returns any and all numbers that look like `UserId` objects as such.
fn get_potential_userids(message: &Message) -> Vec<UserId> {
    message
        .text()
        .map(|text| {
            text.split_whitespace()
                .flat_map(|word| word.parse().map(UserId))
                .collect()
        })
        .unwrap_or_default()
}

/// Looks through the message for any and all users it's directed at, whether by reply or by link
/// mentions. This does not return the sender, the person the message is forwarded from, or users
/// mentioned by username like `@Architector_4`.
fn get_linkable_mentioned_users(message: &Message) -> Vec<&User> {
    let mut output = vec![];
    if let Some(repliee) = message.reply_to_message().and_then(|m| m.from()) {
        output.push(repliee);
    }

    if let Some(ents) = message
        .parse_entities()
        .or_else(|| message.parse_caption_entities())
    {
        for ent in ents {
            use teloxide::types::MessageEntityKind::*;
            match ent.kind() {
                Mention => (), // tough shit lol
                TextMention { user } => output.push(user),
                _ => (), // none other contain users
            }
        }
    }
    output
}

/// Looks through the message and returns all mentions of users by usernames it has like
/// `@Architector_4`. This does not return link mentions (when a user has no username), nor
/// who the message is by, who is it forwarded to, or who it is a reply to.
fn get_text_mentioned_users(message: &Message) -> Vec<&str> {
    let mut output = vec![];
    if let Some(ents) = message
        .parse_entities()
        .or_else(|| message.parse_caption_entities())
    {
        for ent in ents {
            use teloxide::types::MessageEntityKind::*;
            match ent.kind() {
                Mention => output.push(ent.text()),
                TextMention { user: _ } => (), // this does contain a user, but
                // it's not what we are looking for
                _ => (), // no other entity kinds contain a user
            }
        }
    }
    output
}

/// An object that is either a user or an ID of one
pub enum UserLike {
    User(User),
    Id(UserId),
    // Unneeded for my purposes and would just add more pain
    //Username(&'a str)
}

/// Resolves all `UserLike` objects to `User` objects using
/// the specified chat to find users as members of.
pub async fn resolve_to_users(
    what: Vec<UserLike>,
    bot: &Bot,
    chat: &Chat,
) -> Result<Vec<User>, RequestError> {
    futures::future::try_join_all(what.into_iter().map(|ulike| async move {
        match ulike {
            UserLike::User(user) => Ok(user),
            UserLike::Id(uid) => bot.get_chat_member(chat.id, uid).await.map(|m| m.user),
        }
    }))
    .await
}

pub trait MentionResolver {
    /// Log the user's username and ID into this logger.
    fn see_user(&mut self, user: &User);
    /// Resolve a username to a user ID, if one is found in this logger
    fn username_to_userid(&self, name: &str) -> Option<UserId>;

    /// Get all users mentioned by this message that this resolver can resolve.
    /// This skips usernames mentions that are unknown to this resolver.
    fn get_mentioned_users(&self, message: &Message) -> Vec<UserLike> {
        let mut output = vec![];

        for user in get_linkable_mentioned_users(message) {
            output.push(UserLike::User(user.to_owned()))
        }
        for uid in get_potential_userids(message).into_iter().chain(
            get_text_mentioned_users(message)
                .into_iter()
                .flat_map(|mention| self.username_to_userid(mention)),
        ) {
            output.push(UserLike::Id(uid))
            //output.push(bot.get_chat_member(message.chat.id, uid).await?.user)
        }
        output
    }

    /// See and log all users an update event has.
    fn see_users_from_update(&mut self, update: &Update) {
        // Single user extraction
        if let Some(u) = update.user() {
            self.see_user(u)
        }

        match &update.kind {
            UpdateKind::Message(message) | UpdateKind::EditedMessage(message) => {
                // A message has multiple sources of User objects:
                // The one it's from (was already handled);
                // The one it's forwarded from;
                if let Some(u) = message.forward_from_user() {
                    self.see_user(u)
                }
                // The one it's replying to;
                // The ones it mentions by link mentions;
                for u in get_linkable_mentioned_users(message) {
                    self.see_user(u);
                }
                // The ones mentioned by user IDs;
                //get_potential_userids(message)
                // It's possible to resolve those here and log
                // users we find, but that would be too greedy.

                // The ones it mentions by username mentions like `@Architector_4`.
                //get_text_mentioned_users(message);
                // The last one does not give us any new user objects, so it's useless here.
            }
            // The other kinds can only contain a single user and thus were handled by
            // Update::user(&self) call above
            _ => (),
        }
    }
}