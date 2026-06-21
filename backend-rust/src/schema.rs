diesel::table! {
    videos (id) {
        id -> Integer,
        file_path -> Text,
        filename -> Text,
        duration -> Nullable<Float>,
        file_size -> Nullable<BigInt>,
        resolution -> Nullable<Text>,
        fps -> Nullable<Float>,
        codec -> Nullable<Text>,
        created_date -> Nullable<Timestamp>,
        indexed_date -> Timestamp,
        thumbnail_count -> Integer,
        checksum -> Nullable<Text>,
    }
}

diesel::table! {
    metadata (id) {
        id -> Integer,
        video_id -> Integer,
        category -> Nullable<Text>,
        location -> Nullable<Text>,
        notes -> Nullable<Text>,
    }
}

diesel::table! {
    tags (id) {
        id -> Integer,
        name -> Text,
    }
}

diesel::table! {
    video_tags (video_id, tag_id) {
        video_id -> Integer,
        tag_id -> Integer,
    }
}

diesel::table! {
    productions (id) {
        id -> Integer,
        title -> Text,
        platform -> Nullable<Text>,
        link -> Nullable<Text>,
        is_published -> Bool,
    }
}

diesel::table! {
    video_productions (video_id, production_id) {
        video_id -> Integer,
        production_id -> Integer,
    }
}

diesel::table! {
    ai_generations (id) {
        id -> Integer,
        video_id -> Integer,
        transcript -> Nullable<Text>,
        thumbnail_text -> Nullable<Text>,
        instagram_description -> Nullable<Text>,
        tiktok_description -> Nullable<Text>,
        youtube_short_title -> Nullable<Text>,
        youtube_short_description -> Nullable<Text>,
        youtube_short_tags -> Nullable<Text>,
        hashtags -> Nullable<Text>,
        provider -> Nullable<Text>,
        model -> Nullable<Text>,
        generated_at -> Timestamp,
    }
}

diesel::table! {
    production_edits (id) {
        id -> Integer,
        production_id -> Integer,
        status -> Text,
        script -> Nullable<Text>,
        instructions -> Nullable<Text>,
        edl_json -> Nullable<Text>,
        output_path -> Nullable<Text>,
        edl_path -> Nullable<Text>,
        error -> Nullable<Text>,
        transcription_provider -> Nullable<Text>,
        text_provider -> Nullable<Text>,
        text_model -> Nullable<Text>,
        created_at -> Timestamp,
        logs -> Nullable<Text>,
        transcripts_json -> Nullable<Text>,
        options_json -> Nullable<Text>,
        copy_json -> Nullable<Text>,
    }
}

diesel::joinable!(metadata -> videos (video_id));
diesel::joinable!(video_tags -> videos (video_id));
diesel::joinable!(video_tags -> tags (tag_id));
diesel::joinable!(video_productions -> videos (video_id));
diesel::joinable!(video_productions -> productions (production_id));
diesel::joinable!(ai_generations -> videos (video_id));
diesel::joinable!(production_edits -> productions (production_id));

diesel::allow_tables_to_appear_in_same_query!(
    videos,
    metadata,
    tags,
    video_tags,
    productions,
    video_productions,
    ai_generations,
    production_edits,
);
