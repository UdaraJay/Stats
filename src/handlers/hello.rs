use actix_web::{HttpResponse, Responder};

pub async fn welcome() -> impl Responder {
    let art = r#"
 ____ _____  _  _____ ____  
/ ___|_   _|/ \|_   _/ ___| 
\___ \ | | / _ \ | | \___ \ 
 ___) || |/ ___ \| |  ___) |
|____/ |_/_/   \_\_| |____/                       

+ STATS ANALYTICS                                           
+ A minimal analytics provider                                                                  

"#;

    HttpResponse::Ok().body(art)
}
