pub const DEMO_SENTENCE: &str = "㋿Ξ㍾㍿の下北沢\u{E0100}店でnaïveなThé Noirとphởとكَبَابを注文、שָׁלוֹםとनमस्तेで先輩に挨拶した。8月10日、二 人 幸 终。";

pub const LONG_DOCUMENT_TEXT: &str = "\
今天天气非常好，我和朋友一起去了北京旅游。北京是中国的首都，有很多名胜古迹。\
我们参观了故宫和天安门广场，那里的建筑非常壮观。下午我们品尝了北京烤鸭，味道真好。\
The Great Wall of China is one of the most impressive structures in the world. \
We spent the whole afternoon exploring it. \
東京タワーから見た景色は素晴らしかった。日本の文化はとても深いです。\
コンピュータを使って日本語を勉強しています。\
한국어를 공부하는 것은 재미있습니다. 서울은 아름다운 도시입니다. \
Apple recently released the iPhone 15 Pro Max with cutting edge technology. \
Python 3.12 introduced many exciting new features for developers. \
苹果公司推出了最新的产品，搭载最新的芯片。学习编程是一件非常有趣的事情。";

pub struct QueryTestCase {
    pub name: &'static str,
    pub query: &'static str,
    pub matches: &'static [&'static str],
    pub description: &'static str,
}

pub struct QueryTestGroup {
    pub name: &'static str,
    pub docs: &'static [(&'static str, &'static str)],
    pub cases: &'static [QueryTestCase],
}

mod brahmic;
mod chinese_combined;
mod chinese_simple;
mod cjk_normalization;
mod degenerate;
mod japanese;
mod korean;
mod latin;
mod mixed;
mod scoring;
mod semitic;

pub const QUERY_TEST_GROUPS: &[QueryTestGroup] = &[
    chinese_simple::GROUP,
    chinese_combined::GROUP,
    japanese::GROUP,
    korean::GROUP,
    cjk_normalization::GROUP,
    latin::GROUP,
    semitic::GROUP,
    brahmic::GROUP,
    mixed::GROUP,
    degenerate::GROUP,
    scoring::GROUP,
];

/// Very long query for stress testing (1000 copies of 北).
pub fn very_long_query() -> String {
    "北".repeat(1000)
}
